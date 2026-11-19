use crate::core::menu;

pub fn update<Message>(bar: &menu::Bar<Message>) {
    if !bar.is_empty() {
        todo!("native application menus are currently implemented only on macOS");
    }
}
