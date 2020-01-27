/*!
 *  Algorithms to follow a path
 *
 *  A Segment is just one part of a larger path. These can be fed to a Path to follow one
 */

use core::f32::consts::FRAC_PI_2;
use core::f32::consts::PI;

use serde::Deserialize;
use serde::Serialize;

use libm::F32Ext;

use heapless::consts::U16;
use heapless::Vec;
use typenum::Unsigned;

use crate::math::Direction;
use crate::math::Orientation;
use crate::math::Vector;

use crate::bezier::Bezier3;
use crate::bezier::Curve;

/**
 * A segment of a larger path
 *
 * The path following algorithm uses the distance from the path to control steering of the mouse,
 * and the distance along it with the total distance to determine when the segment is complete.
 * The distance along may also be used to control forward velocity
 *
 * Usually, the segments are arranged so that each one starts at the end of the previous one and
 * are tangent. This makes the movement nice and smooth. However, it does not have to be for eg.
 * turning around in place.
 */
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    bezier: Bezier3,
}

impl Segment {
    /// Generate a corner
    ///
    /// # Arguments
    ///
    /// `center`: the location of the corner, where the two lines intersect
    ///
    /// `start`: the absolute direction of the entrance line
    ///
    /// `end`: the absolute direction of the exit line
    ///
    /// `radius` is the distance from the center to the end of each line
    pub fn corner(
        center: Vector,
        start: Direction,
        end: Direction,
        radius: f32,
    ) -> Segment {
        Segment {
            bezier: Bezier3 {
                start: center - radius * start.into_unit_vector(),
                ctrl0: center,
                ctrl1: center,
                end: center + radius * end.into_unit_vector(),
            },
        }
    }

    /// Generate a strait line
    pub fn line(start: Vector, end: Vector) -> Segment {
        let mid = (end - start) * 0.5 + start;
        Segment {
            bezier: Bezier3 {
                start,
                ctrl0: mid,
                ctrl1: mid,
                end,
            },
        }
    }

    /// Find the point on the segment closest to `m`
    pub fn closest_point(&self, m: Vector) -> (f32, Vector) {
        self.bezier.closest_point(m)
    }

    /// Derivative at `t`
    pub fn derivative(&self, t: f32) -> Vector {
        self.bezier.derivative().at(t)
    }

    /// Curvature at `t`
    pub fn curvature(&self, t: f32) -> f32 {
        self.bezier.curvature(t)
    }
}

// Adjust the curvature for the mouse not being on the path
fn offset_curvature(curvature: f32, distance: f32) -> f32 {
    let r = 1.0 / curvature;

    let r2 = if curvature > 0.0 {
        r - distance
    } else {
        r + distance
    };

    let curvature2 = 1.0 / r2;
    curvature2
}

#[cfg(test)]
mod offset_curvature_tests {
    use super::offset_curvature;
    #[allow(unused_imports)]
    use crate::test::*;

    #[test]
    fn zero_distance_positive_curvature() {
        assert_close(offset_curvature(1.0, 0.0), 1.0)
    }

    #[test]
    fn positive_distance_positive_curvature() {
        assert_close(offset_curvature(1.0, 0.5), 2.0)
    }

    #[test]
    fn negative_distance_positive_curvature() {
        assert_close(offset_curvature(1.0, -0.5), 0.6666667)
    }

    #[test]
    fn zero_distance_negative_curvature() {
        assert_close(offset_curvature(-1.0, 0.0), -1.0)
    }

    #[test]
    fn positive_distance_negative_curvature() {
        assert_close(offset_curvature(-1.0, 0.5), -2.0)
    }

    #[test]
    fn negative_distance_negative_curvature() {
        assert_close(offset_curvature(-1.0, -0.5), -0.66666667)
    }

    #[test]
    fn zero_curvature() {
        assert_close(offset_curvature(0.0, 0.5), 0.0)
    }
}

pub type PathBufLen = U16;
pub type PathBuf = Vec<Segment, PathBufLen>;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PathDebug {
    pub path: Option<PathBuf>,
    pub closest_point: Option<(f32, Vector)>,
    pub distance_from: Option<f32>,
    pub tangent_direction: Option<Direction>,
    pub adjust_direction: Option<Direction>,
    pub centered_direction: Option<f32>,
    pub offset_direction: Option<f32>,
    pub projected_distance: Option<f32>,
    pub adjust_curvature: Option<f32>,
    pub target_curvature: Option<f32>,
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PathConfig {
    pub offset_p: f32,
    pub velocity: f32,
}

#[derive(Clone, Debug)]
pub struct Path {
    pub segment_buffer: PathBuf,
    pub time: u32,
}

impl Path {
    pub fn new(_config: &PathConfig, time: u32) -> Path {
        Path {
            segment_buffer: Vec::new(),
            time,
        }
    }

    pub fn add_segments(&mut self, segments: &[Segment]) -> Result<usize, usize> {
        for (i, segment) in segments.iter().enumerate() {
            if self.segment_buffer.push(*segment).is_err() {
                return Err(i);
            }
        }

        Ok(PathBufLen::to_usize() - self.segment_buffer.len())
    }

    pub fn update(
        &mut self,
        config: &PathConfig,
        time: u32,
        orientation: Orientation,
    ) -> (f32, f32, bool, PathDebug) {
        let mut debug = PathDebug::default();

        let delta_time = time - self.time;

        // Go through the buffer and pop off any moves that have been completed, and get the info
        // for the first that is not completed, or None if there are no more moves
        let segment_info = loop {
            if let Some(segment) = self.segment_buffer.last() {
                let (t, p) = segment.closest_point(orientation.position);
                debug.closest_point = Some((t, p));
                if t >= 1.0 {
                    self.segment_buffer.pop();
                    continue;
                } else {
                    let v_tangent = segment.derivative(t);
                    let v_m = orientation.position - p;
                    let distance = if v_tangent.cross(v_m) > 0.0 {
                        v_m.magnitude()
                    } else {
                        -v_m.magnitude()
                    };

                    let tangent = v_tangent.direction();

                    let curvature = segment.curvature(t);

                    break Some((curvature, distance, tangent));
                }
            } else {
                break None;
            }
        };

        // If there was another segment, try to follow it
        let (curvature, velocity, done) =
            if let Some((path_curvature, distance, tangent)) = segment_info {
                // The curvature of the path where the mouse is
                let offset_curvature = offset_curvature(path_curvature, distance);

                let adjust_curvature = if config.offset_p != 0.0 {
                    // Need to calculate an adjustment curvature to get the mouse back on the path
                    // This gets added to the offset curvature above to get the final path curvature.
                    // As such, it should always turn the mouse towards the path, but avoid turning
                    // past the path. This is done by calculating a target direction that points towards the
                    // path far away, but along the path close up. A curvature is then calculated that
                    // should get the mouse to that direction in the next loop (assuming no physics
                    // limitations. This should probably be limited base on the mechanics).

                    // This s-curve will asymptote at -pi/2 and pi/2, and cross the origin.
                    // Points the mouse directly at the path far away, but along the path
                    // close up. The offset_p determines how aggressive it is
                    let adjust_direction_offset =
                        PI / (1.0 + F32Ext::exp(config.offset_p * distance)) - FRAC_PI_2;

                    let adjust_direction =
                        tangent + Direction::from(adjust_direction_offset);

                    let projected_distance = delta_time as f32 * config.velocity;

                    let centered_direction =
                        orientation.direction.centered_at(adjust_direction);

                    let offset_direction =
                        f32::from(adjust_direction) - centered_direction;

                    debug.adjust_direction = Some(adjust_direction);
                    debug.centered_direction = Some(centered_direction);
                    debug.offset_direction = Some(offset_direction);
                    debug.projected_distance = Some(projected_distance);

                    // Curvature can be measured in radians per mm.
                    // Reasoning:
                    // The arclength of a circular arc is the radius times the angle in radians. Thus
                    // the radius is the arclength divided by the angle. If the arclength is in mm, then
                    // the radius is in mm per radian. Curvature is the inverse of the radius, so it is
                    // radians per mm.
                    offset_direction / projected_distance
                } else {
                    0.0
                };

                let target_curvature = offset_curvature + adjust_curvature;

                debug.distance_from = Some(distance);
                debug.tangent_direction = Some(tangent);
                debug.adjust_curvature = Some(adjust_curvature);
                debug.target_curvature = Some(target_curvature);

                (target_curvature, config.velocity, false)
            } else {
                (0.0, 0.0, true)
            };

        debug.path = Some(self.segment_buffer.clone());

        self.time = time;

        (curvature, velocity, done, debug)
    }
}
