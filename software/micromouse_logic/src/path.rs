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

use pid_control::Controller;
use pid_control::DerivativeMode;
use pid_control::PIDController;

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

pub type PathBufLen = U16;
pub type PathBuf = Vec<Segment, PathBufLen>;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PathDebug {
    pub path: Option<PathBuf>,
    pub closest_point: Option<(f32, Vector)>,
    pub distance_from: Option<f32>,
    pub centered_direction: Option<f32>,
    pub tangent_direction: Option<Direction>,
    pub target_direction: Option<Direction>,
    pub target_direction_offset: Option<f32>,
    pub error: Option<f32>,
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PathConfig {
    pub p: f32,
    pub i: f32,
    pub d: f32,
    pub offset_p: f32,
}

#[derive(Clone, Debug)]
pub struct Path {
    pub pid: PIDController,
    pub segment_buffer: PathBuf,
    pub time: u32,
}

impl Path {
    pub fn new(config: &PathConfig, time: u32) -> Path {
        let mut pid = PIDController::new(
            config.p as f64,
            config.i as f64,
            config.d as f64,
        );
        pid.d_mode = DerivativeMode::OnError;
        //pid.set_limits(-1.0, 1.0);
        Path {
            pid,
            segment_buffer: Vec::new(),
            time,
        }
    }

    pub fn add_segments(
        &mut self,
        segments: &[Segment],
    ) -> Result<usize, usize> {
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
    ) -> (f32, bool, PathDebug) {
        let mut debug = PathDebug {
            path: None,
            closest_point: None,
            distance_from: None,
            centered_direction: None,
            tangent_direction: None,
            target_direction: None,
            target_direction_offset: None,
            error: None,
        };

        self.pid.p_gain = config.p as f64;
        self.pid.i_gain = config.i as f64;
        self.pid.d_gain = config.d as f64;

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
        let (curvature, done) =
            if let Some((curvature, distance, _tangent)) = segment_info {
                if curvature == 0.0 {
                    (0.0, false)
                } else {
                    // Adjust the curvature for the mouse not being on the path
                    let r = 1.0 / curvature;
                    if distance > 0.0 {
                        let r2 = r + distance;
                        let curvature2 = 1.0 / r2;
                        (curvature2, false)
                    } else {
                        let r2 = r - distance;
                        let curvature2 = 1.0 / r2;
                        (curvature2, false)
                    }
                }
            } else {
                (0.0, true)
            };

        debug.path = Some(self.segment_buffer.clone());

        (curvature, done, debug)
    }
}
