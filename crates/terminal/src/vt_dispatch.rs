//! Commands routed from the IO thread to the VT emulator thread.

pub enum VtCommand {
    Resize {
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    },
    Scroll(isize),
    ScrollToBottom,
}
