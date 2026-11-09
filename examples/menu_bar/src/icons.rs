use iced::menu;
use iced::widget::image;

pub struct Icons {
    pub increase: menu::Icon,
    pub decrease: menu::Icon,
    pub reset: image::Handle,
}

impl Default for Icons {
    fn default() -> Self {
        Self {
            increase: template(|x, y| (14..18).contains(&x) || (14..18).contains(&y)),
            decrease: template(|_, y| (14..18).contains(&y)),
            reset: image::Handle::from_bytes(
                include_bytes!("../../tour/images/ferris.png").as_slice(),
            ),
        }
    }
}

fn template(contains: impl Fn(u32, u32) -> bool) -> menu::Icon {
    let mut pixels = vec![0_u8; 32 * 32 * 4];
    for y in 5..27 {
        for x in 5..27 {
            if contains(x, y) {
                pixels[((y * 32 + x) * 4 + 3) as usize] = 255;
            }
        }
    }
    menu::Icon::new(image::Handle::from_rgba(32, 32, pixels)).template(true)
}
