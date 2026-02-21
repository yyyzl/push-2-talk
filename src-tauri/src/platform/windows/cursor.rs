#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(lp_point: *mut Point) -> i32;
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

pub fn get_cursor_position() -> Option<(i32, i32)> {
    let mut point = Point { x: 0, y: 0 };
    unsafe {
        if GetCursorPos(&mut point) != 0 {
            Some((point.x, point.y))
        } else {
            None
        }
    }
}
