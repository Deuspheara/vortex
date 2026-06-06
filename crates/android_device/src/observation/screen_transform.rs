use agent_protocol::AndroidPointPx;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenTransform {
    pub device_width: f32,
    pub device_height: f32,
    pub view_width: f32,
    pub view_height: f32,
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

impl ScreenTransform {
    pub fn new(device_width: f32, device_height: f32, view_width: f32, view_height: f32) -> Self {
        let scale = (view_width / device_width).min(view_height / device_height);
        let offset_x = (view_width - device_width * scale) / 2.0;
        let offset_y = (view_height - device_height * scale) / 2.0;
        Self {
            device_width,
            device_height,
            view_width,
            view_height,
            scale,
            offset_x,
            offset_y,
        }
    }

    pub fn device_to_view(&self, point: AndroidPointPx) -> AndroidPointPx {
        AndroidPointPx {
            x: self.offset_x + point.x * self.scale,
            y: self.offset_y + point.y * self.scale,
        }
    }

    pub fn view_to_device(&self, point: AndroidPointPx) -> AndroidPointPx {
        AndroidPointPx {
            x: ((point.x - self.offset_x) / self.scale).clamp(0.0, self.device_width),
            y: ((point.y - self.offset_y) / self.scale).clamp(0.0, self.device_height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_letterboxed_points_both_ways() {
        let tx = ScreenTransform::new(1000.0, 2000.0, 500.0, 500.0);
        assert_eq!(tx.scale, 0.25);
        assert_eq!(tx.offset_x, 125.0);
        let view = tx.device_to_view(AndroidPointPx {
            x: 500.0,
            y: 1000.0,
        });
        assert_eq!(view, AndroidPointPx { x: 250.0, y: 250.0 });
        let device = tx.view_to_device(view);
        assert_eq!(
            device,
            AndroidPointPx {
                x: 500.0,
                y: 1000.0
            }
        );
    }
}
