use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icon(&'static str);

pub const FOLDER_OPEN: Icon = Icon(
    r#"<path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"></path>"#,
);
pub const REDO_2: Icon = Icon(
    r#"<path d="m15 14 5-5-5-5"></path><path d="M20 9H9.5A5.5 5.5 0 0 0 4 14.5A5.5 5.5 0 0 0 9.5 20H13"></path>"#,
);
pub const SEND: Icon = Icon(
    r#"<path d="M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z"></path><path d="m21.854 2.147-10.94 10.939"></path>"#,
);
pub const UNDO_2: Icon = Icon(
    r#"<path d="M9 14 4 9l5-5"></path><path d="M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11"></path>"#,
);
pub const HISTORY: Icon = Icon(
    r#"<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"></path><path d="M3 3v5h5"></path><path d="M12 7v5l4 2"></path>"#,
);
pub const ARCHIVE: Icon = Icon(
    r#"<rect width="20" height="5" x="2" y="3" rx="1"></rect><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"></path><path d="M10 12h4"></path>"#,
);
pub const ARCHIVE_RESTORE: Icon = Icon(
    r#"<rect width="20" height="5" x="2" y="3" rx="1"></rect><path d="M4 8v11a2 2 0 0 0 2 2h2"></path><path d="M20 8v11a2 2 0 0 1-2 2h-2"></path><path d="m9 15 3-3 3 3"></path><path d="M12 12v9"></path>"#,
);
pub const CHECK: Icon = Icon(r#"<path d="M20 6 9 17l-5-5"></path>"#);
pub const IMAGE_OFF: Icon = Icon(
    r#"<line x1="2" x2="22" y1="2" y2="22"></line><path d="M10.41 10.41a2 2 0 1 1-2.83-2.83"></path><line x1="13.5" x2="6" y1="13.5" y2="21"></line><line x1="18" x2="21" y1="12" y2="15"></line><path d="M3.59 3.59A1.99 1.99 0 0 0 3 5v14a2 2 0 0 0 2 2h14c.55 0 1.052-.22 1.41-.59"></path><path d="M21 15V5a2 2 0 0 0-2-2H9"></path>"#,
);
pub const SLIDERS_HORIZONTAL: Icon = Icon(
    r#"<path d="M10 5H3"></path><path d="M12 19H3"></path><path d="M14 3v4"></path><path d="M16 17v4"></path><path d="M21 12h-9"></path><path d="M21 19h-5"></path><path d="M21 5h-7"></path><path d="M8 10v4"></path><path d="M8 12H3"></path>"#,
);
pub const ARROW_DOWN_AZ: Icon = Icon(
    r#"<path d="m3 16 4 4 4-4"></path><path d="M7 20V4"></path><path d="M20 8h-5"></path><path d="M15 10V6.5a2.5 2.5 0 0 1 5 0V10"></path><path d="M15 14h5l-5 6h5"></path>"#,
);
pub const CALENDAR_CLOCK: Icon = Icon(
    r#"<path d="M16 14v2.2l1.6 1"></path><path d="M16 2v3"></path><path d="M21 7.338V5a2 2 0 00-2-2H5a2 2 0 00-2 2v14a2 2 0 002 2h2.338"></path><path d="M3 9h5.859"></path><path d="M8 2v3"></path><circle cx="16" cy="16" r="6"></circle>"#,
);
pub const GRID_2X2: Icon = Icon(
    r#"<path d="M12 3v18"></path><path d="M3 12h18"></path><rect x="3" y="3" width="18" height="18" rx="2"></rect>"#,
);
pub const LIST: Icon = Icon(
    r#"<path d="M3 5h.01"></path><path d="M3 12h.01"></path><path d="M3 19h.01"></path><path d="M8 5h13"></path><path d="M8 12h13"></path><path d="M8 19h13"></path>"#,
);
pub const ROWS_3: Icon = Icon(
    r#"<rect width="18" height="18" x="3" y="3" rx="2"></rect><path d="M21 9H3"></path><path d="M21 15H3"></path>"#,
);
pub const SQUARE_PLUS: Icon = Icon(
    r#"<rect width="18" height="18" x="3" y="3" rx="2"></rect><path d="M8 12h8"></path><path d="M12 8v8"></path>"#,
);
pub const X: Icon = Icon(r#"<path d="M18 6 6 18"></path><path d="m6 6 12 12"></path>"#);
pub const ARROW_DOWN_WIDE_NARROW: Icon = Icon(
    r#"<path d="m3 16 4 4 4-4"></path><path d="M7 20V4"></path><path d="M11 4h10"></path><path d="M11 8h7"></path><path d="M11 12h4"></path>"#,
);
pub const COPY: Icon = Icon(
    r#"<rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path>"#,
);
pub const DOWNLOAD: Icon = Icon(
    r#"<path d="M12 15V3"></path><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><path d="m7 10 5 5 5-5"></path>"#,
);
pub const EYE_OFF: Icon = Icon(
    r#"<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"></path><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"></path><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"></path><path d="m2 2 20 20"></path>"#,
);
pub const GRIP_VERTICAL: Icon = Icon(
    r#"<circle cx="9" cy="12" r="1"></circle><circle cx="9" cy="5" r="1"></circle><circle cx="9" cy="19" r="1"></circle><circle cx="15" cy="12" r="1"></circle><circle cx="15" cy="5" r="1"></circle><circle cx="15" cy="19" r="1"></circle>"#,
);
pub const PLUS: Icon = Icon(r#"<path d="M5 12h14"></path><path d="M12 5v14"></path>"#);
pub const TRASH_2: Icon = Icon(
    r#"<path d="M10 11v6"></path><path d="M14 11v6"></path><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"></path><path d="M3 6h18"></path><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>"#,
);
pub const CHEVRON_LEFT: Icon = Icon(r#"<path d="m15 18-6-6 6-6"></path>"#);
pub const CHEVRON_RIGHT: Icon = Icon(r#"<path d="m9 18 6-6-6-6"></path>"#);
pub const CLIPBOARD_COPY: Icon = Icon(
    r#"<rect width="8" height="4" x="8" y="2" rx="1" ry="1"></rect><path d="M8 4H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2"></path><path d="M16 4h2a2 2 0 0 1 2 2v4"></path><path d="M21 14H11"></path><path d="m15 10-4 4 4 4"></path>"#,
);
pub const CLIPBOARD_PASTE: Icon = Icon(
    r#"<path d="M11 14h10"></path><path d="M16 4h2a2 2 0 0 1 2 2v1.344"></path><path d="m17 18 4-4-4-4"></path><path d="M8 4H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 1.793-1.113"></path><rect x="8" y="2" width="8" height="4" rx="1"></rect>"#,
);
pub const CROP: Icon =
    Icon(r#"<path d="M6 2v14a2 2 0 0 0 2 2h14"></path><path d="M18 22V8a2 2 0 0 0-2-2H2"></path>"#);
pub const EYE: Icon = Icon(
    r#"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"></path><circle cx="12" cy="12" r="3"></circle>"#,
);
pub const FLIP_HORIZONTAL_2: Icon = Icon(
    r#"<path d="M10 12H8"></path><path d="M16 12h-2"></path><path d="M22 12h-2"></path><path d="M4 12H2"></path><path d="M7.298 20.288A1 1 0 008 22h8a1 1 0 00.703-1.712l-3.991-3.99a1 1 0 00-1.424-.001z"></path><path d="M7.298 3.712A1 1 0 018 2h8a1 1 0 01.703 1.712l-3.991 3.99a1 1 0 01-1.424.001z"></path>"#,
);
pub const ROTATE_CCW: Icon = Icon(
    r#"<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"></path><path d="M3 3v5h5"></path>"#,
);
pub const ROTATE_CW: Icon = Icon(
    r#"<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"></path><path d="M21 3v5h-5"></path>"#,
);

pub fn svg(icon: Icon, class: &'static str) -> impl IntoView {
    stroked(icon, class, "2")
}

pub fn stroked(icon: Icon, class: &'static str, stroke_width: &'static str) -> impl IntoView {
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width=stroke_width
            stroke-linecap="round"
            stroke-linejoin="round"
            class=class
            aria-hidden="true"
            inner_html=icon.0
        ></svg>
    }
}
