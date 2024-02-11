use sdl2::rect::Rect;

pub trait Layout: Sized {
    fn scroll_y(self, scroll_distance: i32) -> Self;

    fn split_grid_center(
        self,
        width: u32,
        height: u32,
        x_pad: i32,
        y_pad: i32,
    ) -> (usize, impl Iterator<Item = Self>);

    fn split_grid(
        self,
        width: u32,
        height: u32,
        x_pad: i32,
        y_pad: i32,
    ) -> (usize, impl Iterator<Item = Self>);

    fn split_hori(self, top: u32, ratio: u32) -> (Self, Self);

    fn split_vert(self, left: u32, ratio: u32) -> (Self, Self);

    fn split_even_hori(self, height: u32) -> impl Iterator<Item = Self>;

    fn overlay_vert(self, top: u32, ratio: u32) -> (Self, Self);

    fn pad_outer(self, pad_x: u32, pad_y: u32) -> Self;

    fn pad_left(self, pad: i32) -> Self;

    fn pad_right(self, pad: i32) -> Self;

    fn pad_top(self, pad: i32) -> Self;

    fn pad_bottom(self, pad: i32) -> Self;
}

impl Layout for Rect {
    fn scroll_y(self, scroll_distance: i32) -> Self {
        Self::new(
            self.x,
            self.y + scroll_distance,
            self.width(),
            self.height(),
        )
    }

    fn split_grid_center(
        mut self,
        width: u32,
        height: u32,
        x_pad: i32,
        y_pad: i32,
    ) -> (usize, impl Iterator<Item = Self>) {
        self.set_width(self.width() + x_pad as u32);
        let wrap_width = self.width();
        let idx_wrap = (wrap_width as i32 - self.x) / (width as i32 + x_pad);
        let max_width = (width as i32 + x_pad) * idx_wrap;
        self.x = (wrap_width as i32 - max_width) / 2;
        self.split_grid(width, height, x_pad, y_pad)
    }

    fn split_grid(
        self,
        width: u32,
        height: u32,
        x_pad: i32,
        y_pad: i32,
    ) -> (usize, impl Iterator<Item = Self>) {
        let wrap_width = self.width();
        let idx_wrap = (wrap_width as i32 - self.x) / (width as i32 + x_pad);
        (
            idx_wrap as usize,
            (0..).map(move |idx| {
                Self::new(
                    self.x + (idx as i32 % idx_wrap * (width as i32 + x_pad)),
                    self.y + (height as i32 + y_pad) * (idx as i32 / idx_wrap),
                    width,
                    height,
                )
            }),
        )
    }

    fn split_hori(self, top: u32, ratio: u32) -> (Self, Self) {
        assert!(top < ratio);
        let top_height = (self.height() as f32 * (top as f32 / ratio as f32)) as u32;
        let top_layout = Self::new(self.x, self.y, self.width(), top_height);
        let bottom_layout = Self::new(
            self.x,
            self.y + top_height as i32,
            self.width(),
            self.height() - top_height,
        );
        (top_layout, bottom_layout)
    }

    fn split_vert(self, left: u32, ratio: u32) -> (Self, Self) {
        assert!(left < ratio);
        let left_width = (self.width() as f32 * (left as f32 / ratio as f32)) as u32;
        let left_layout = Self::new(self.x, self.y, left_width, self.height());
        let right_layout = Self::new(
            self.x + left_width as i32,
            self.y,
            self.width() - left_width,
            self.height(),
        );
        (left_layout, right_layout)
    }

    fn split_even_hori(self, height: u32) -> impl Iterator<Item = Self> {
        (0..).map(move |idx| {
            Self::new(
                self.x,
                self.y + height as i32 * idx as i32,
                self.width(),
                height,
            )
        })
    }

    fn overlay_vert(self, top: u32, ratio: u32) -> (Self, Self) {
        assert!(top < ratio);
        let top_height = (self.height() as f32 * (top as f32 / ratio as f32)) as u32;
        let top_layout = self;
        let bottom_layout = Self::new(
            self.x,
            self.y + top_height as i32,
            self.width(),
            self.height() - top_height,
        );
        (top_layout, bottom_layout)
    }

    fn pad_outer(self, pad_x: u32, pad_y: u32) -> Self {
        Self::new(
            self.x + pad_x as i32,
            self.y + pad_y as i32,
            self.width() - 2 * pad_x,
            self.height() - 2 * pad_y,
        )
    }

    fn pad_left(self, pad: i32) -> Self {
        Self::new(self.x + pad, self.y, self.width(), self.height())
    }

    fn pad_right(self, pad: i32) -> Self {
        Self::new(
            self.x,
            self.y,
            (self.width() as i32 - pad * 2) as u32,
            self.height(),
        )
    }

    fn pad_top(self, pad: i32) -> Self {
        Self::new(self.x, self.y + pad, self.width(), self.height())
    }

    fn pad_bottom(self, pad: i32) -> Self {
        Self::new(
            self.x,
            self.y,
            self.width(),
            (self.height() as i32 - pad) as u32,
        )
    }
}

