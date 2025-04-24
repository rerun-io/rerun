DEFAULT LIGHT THEME: Visuals {
    dark_mode: false,
    override_text_color: None,
    widgets: Widgets {
        noninteractive: WidgetVisuals {
            bg_fill: #F8_F8_F8_FF,
            weak_bg_fill: #F8_F8_F8_FF,
            bg_stroke: Stroke {
                width: 1.0,
                color: #BE_BE_BE_FF,
            },
            corner_radius: CornerRadius {
                nw: 2,
                ne: 2,
                sw: 2,
                se: 2,
            },
            fg_stroke: Stroke {
                width: 1.0,
                color: #50_50_50_FF,
            },
            expansion: 0.0,
        },
        inactive: WidgetVisuals {
            bg_fill: #E6_E6_E6_FF,
            weak_bg_fill: #E6_E6_E6_FF,
            bg_stroke: Stroke {
                width: 0.0,
                color: #00_00_00_00,
            },
            corner_radius: CornerRadius {
                nw: 2,
                ne: 2,
                sw: 2,
                se: 2,
            },
            fg_stroke: Stroke {
                width: 1.0,
                color: #3C_3C_3C_FF,
            },
            expansion: 0.0,
        },
        hovered: WidgetVisuals {
            bg_fill: #DC_DC_DC_FF,
            weak_bg_fill: #DC_DC_DC_FF,
            bg_stroke: Stroke {
                width: 1.0,
                color: #69_69_69_FF,
            },
            corner_radius: CornerRadius {
                nw: 3,
                ne: 3,
                sw: 3,
                se: 3,
            },
            fg_stroke: Stroke {
                width: 1.5,
                color: #00_00_00_FF,
            },
            expansion: 1.0,
        },
        active: WidgetVisuals {
            bg_fill: #A5_A5_A5_FF,
            weak_bg_fill: #A5_A5_A5_FF,
            bg_stroke: Stroke {
                width: 1.0,
                color: #00_00_00_FF,
            },
            corner_radius: CornerRadius {
                nw: 2,
                ne: 2,
                sw: 2,
                se: 2,
            },
            fg_stroke: Stroke {
                width: 2.0,
                color: #00_00_00_FF,
            },
            expansion: 1.0,
        },
        open: WidgetVisuals {
            bg_fill: #DC_DC_DC_FF,
            weak_bg_fill: #DC_DC_DC_FF,
            bg_stroke: Stroke {
                width: 1.0,
                color: #A0_A0_A0_FF,
            },
            corner_radius: CornerRadius {
                nw: 2,
                ne: 2,
                sw: 2,
                se: 2,
            },
            fg_stroke: Stroke {
                width: 1.0,
                color: #00_00_00_FF,
            },
            expansion: 0.0,
        },
    },
    selection: Selection {
        bg_fill: #90_D1_FF_FF,
        stroke: Stroke {
            width: 1.0,
            color: #00_53_7D_FF,
        },
    },
    hyperlink_color: #00_9B_FF_FF,
    faint_bg_color: #05_05_05_00,
    extreme_bg_color: #FF_FF_FF_FF,
    code_bg_color: #E6_E6_E6_FF,
    warn_fg_color: #FF_64_00_FF,
    error_fg_color: #FF_00_00_FF,
    window_corner_radius: CornerRadius {
        nw: 6,
        ne: 6,
        sw: 6,
        se: 6,
    },
    window_shadow: Shadow {
        offset: [
            10,
            20,
        ],
        blur: 15,
        spread: 0,
        color: #00_00_00_19,
    },
    window_fill: #F8_F8_F8_FF,
    window_stroke: Stroke {
        width: 1.0,
        color: #BE_BE_BE_FF,
    },
    window_highlight_topmost: true,
    menu_corner_radius: CornerRadius {
        nw: 6,
        ne: 6,
        sw: 6,
        se: 6,
    },
    panel_fill: #F8_F8_F8_FF,
    popup_shadow: Shadow {
        offset: [
            6,
            10,
        ],
        blur: 8,
        spread: 0,
        color: #00_00_00_19,
    },
    resize_corner_size: 12.0,
    text_cursor: TextCursorStyle {
        stroke: Stroke {
            width: 2.0,
            color: #00_53_7D_FF,
        },
        preview: false,
        blink: true,
        on_duration: 0.5,
        off_duration: 0.5,
    },
    clip_rect_margin: 3.0,
    button_frame: true,
    collapsing_header_frame: false,
    indent_has_left_vline: true,
    striped: false,
    slider_trailing_fill: false,
    handle_shape: Circle,
    interact_cursor: None,
    image_loading_spinners: true,
    numeric_color_space: GammaByte,
}



CUSTOMIZED THEME: Visuals {
    dark_mode: false,
    override_text_color: None,
    widgets: Widgets {
        noninteractive: WidgetVisuals {
            bg_fill: #DB_DE_E4_FF, // ✨NEW!✨
            weak_bg_fill: #8D_8F_95_FF, // ✨NEW!✨
            bg_stroke: Stroke {
                width: 1.0,
                color: #DB_DE_E4_FF, // ✨NEW!✨
            },
            corner_radius: CornerRadius { // ✨NEW!✨
                nw: 4,
                ne: 4,
                sw: 4,
                se: 4,
            },
            fg_stroke: Stroke {
                width: 1.0,
                color: #7E_80_86_FF, // ✨NEW!✨
            },
            expansion: 0.0,
        },
        inactive: WidgetVisuals {
            bg_fill: #DB_DE_E4_FF, // ✨NEW!✨
            weak_bg_fill: #00_00_00_00, // ✨NEW!✨
            bg_stroke: Stroke {
                width: 0.0,
                color: #00_00_00_00,
            },
            corner_radius: CornerRadius { // ✨NEW!✨
                nw: 4,
                ne: 4,
                sw: 4,
                se: 4,
            },
            fg_stroke: Stroke {
                width: 1.0,
                color: #38_3A_3F_FF, // ✨NEW!✨
            },
            expansion: 0.0,
        },
        hovered: WidgetVisuals {
            bg_fill: #BB_BE_C4_FF, // ✨NEW!✨
            weak_bg_fill: #DB_DE_E4_FF, // ✨NEW!✨
            bg_stroke: Stroke { // ✨NEW!✨
                width: 0.0,
                color: #00_00_00_00,
            },
            corner_radius: CornerRadius { // ✨NEW!✨
                nw: 4,
                ne: 4,
                sw: 4,
                se: 4,
            },
            fg_stroke: Stroke {
                width: 1.5,
                color: #00_00_00_FF,
            },
            expansion: 1.0,
        },
        active: WidgetVisuals {
            bg_fill: #AB_AE_B4_FF, // ✨NEW!✨
            weak_bg_fill: #CB_CE_D4_FF, // ✨NEW!✨
            bg_stroke: Stroke { // ✨NEW!✨
                width: 0.0,
                color: #00_00_00_00,
            },
            corner_radius: CornerRadius { // ✨NEW!✨
                nw: 4,
                ne: 4,
                sw: 4,
                se: 4,
            },
            fg_stroke: Stroke {
                width: 2.0,
                color: #03_03_05_FF, // ✨NEW!✨
            },
            expansion: 1.0,
        },
        open: WidgetVisuals {
            bg_fill: #DC_DC_DC_FF,
            weak_bg_fill: #AB_AE_B4_FF,
            bg_stroke: Stroke { // ✨NEW!✨
                width: 0.0,
                color: #00_00_00_00,
            },
            corner_radius: CornerRadius { // ✨NEW!✨
                nw: 4,
                ne: 4,
                sw: 4,
                se: 4,
            },
            fg_stroke: Stroke {
                width: 1.0,
                color: #00_00_00_FF,
            },
            expansion: 0.0,
        },
    },
    selection: Selection {
        bg_fill: #2C_56_FF_FF,// ✨NEW!✨
        stroke: Stroke {// ✨NEW!✨
            width: 2.0,
            color: #D0_DE_FF_FF,
        },
    },
    hyperlink_color: #38_3A_3F_FF, // ✨NEW!✨
    faint_bg_color: #EC_EE_F5_FF, // ✨NEW!✨
    extreme_bg_color: #EC_EE_F5_FF, // ✨NEW!✨
    code_bg_color: #E6_E6_E6_FF,
    warn_fg_color: #FF_7A_0C_FF, // ✨NEW!✨
    error_fg_color: #AB_01_16_FF, // ✨NEW!✨
    window_corner_radius: CornerRadius { // ✨NEW!✨
        nw: 12,
        ne: 12,
        sw: 12,
        se: 12,
    },
    window_shadow: Shadow { // ✨NEW!✨
        offset: [
            0,
            15,
        ],
        blur: 50,
        spread: 0,
        color: #00_00_00_20,
    },
    window_fill: #FB_FC_FF_FF, // ✨NEW!✨
    window_stroke: Stroke { // ✨NEW!✨
        width: 0.0,
        color: #00_00_00_00,
    },
    window_highlight_topmost: true,
    menu_corner_radius: CornerRadius { // ✨NEW!✨
        nw: 12,
        ne: 12,
        sw: 12,
        se: 12,
    },
    panel_fill: #FB_FC_FF_FF, // ✨NEW!✨
    popup_shadow: Shadow { // ✨NEW!✨
        offset: [
            0,
            15,
        ],
        blur: 50,
        spread: 0,
        color: #00_00_00_20,
    },
    resize_corner_size: 12.0,
    text_cursor: TextCursorStyle {
        stroke: Stroke {
            width: 2.0,
            color: #00_53_7D_FF,
        },
        preview: false,
        blink: true,
        on_duration: 0.5,
        off_duration: 0.5,
    },
    clip_rect_margin: 0.0, // ✨NEW!✨
    button_frame: true,
    collapsing_header_frame: false,
    indent_has_left_vline: false, // ✨NEW!✨
    striped: false,
    slider_trailing_fill: false,
    handle_shape: Circle,
    interact_cursor: None,
    image_loading_spinners: false, // ✨NEW!✨
    numeric_color_space: GammaByte,
}
