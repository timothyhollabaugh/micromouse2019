use core::f32;

use crate::config::MouseConfig;
use crate::map::Map;
use crate::map::Orientation;
use crate::map::Vector;
use crate::path::Path;
use crate::path::PathDebug;
use crate::path::Segment;

#[derive(Debug)]
pub struct MouseDebug<'a> {
    pub orientation: Orientation,
    pub path_debug: PathDebug<'a>,
}

pub struct Mouse {
    map: Map,
    path: Path,
    done: bool,
}

impl Mouse {
    pub fn new(
        config: &MouseConfig,
        orientation: Orientation,
        time: u32,
        left_encoder: i32,
        right_encoder: i32,
    ) -> Mouse {
        let mut path = Path::new(&config.path, time);

        path.add_segments(&[
            Segment::Line(
                Vector {
                    x: 1000.0,
                    y: 1000.0,
                },
                Vector {
                    x: 2000.0,
                    y: 1000.0,
                },
            ),
            /*
            Segment::Line(
                Vector {
                    x: 720.0 + 90.0,
                    y: 720.0 + 360.0,
                },
                Vector {
                    x: 720.0 + 360.0,
                    y: 720.0 + 360.0,
                },
            ),
            Segment::Line(
                Vector {
                    x: 720.0 + 360.0,
                    y: 720.0 + 360.0,
                },
                Vector {
                    x: 720.0 + 360.0,
                    y: 720.0 + 90.0,
                },
            ),
            Segment::Line(
                Vector {
                    x: 720.0 + 360.0,
                    y: 720.0 + 90.0,
                },
                Vector {
                    x: 720.0 + 90.0,
                    y: 720.0 + 90.0,
                },
            ),
            */
        ]);

        Mouse {
            map: Map::new(orientation, left_encoder, right_encoder),
            path,
            done: true,
        }
    }

    pub fn update(
        &mut self,
        config: &MouseConfig,
        time: u32,
        left_encoder: i32,
        right_encoder: i32,
    ) -> (f32, f32, MouseDebug) {
        let orientation = self
            .map
            .update(&config.mechanical, left_encoder, right_encoder);

        let (angular_power, done, path_debug) =
            self.path.update(&config.path, time, orientation.position);

        self.done = done;

        let linear_power = if done { 0.0 } else { 0.5 };

        let left_power = linear_power - angular_power;
        let right_power = linear_power + angular_power;

        let debug = MouseDebug {
            orientation,
            path_debug,
        };

        (left_power, right_power, debug)
    }
}
