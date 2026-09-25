use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, DrawingArea, Label, Orientation};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use lychnos_core::{
    diagnostics::DiagnosticLevel,
    presentation::{
        CompanionPresentationState, DiagnosticPresentation, PendingApprovalPresentation,
    },
    runtime::RuntimeMode,
};

const APP_ID: &str = "org.lychnos.prototype.shell";

fn main() {
    let application = Application::builder().application_id(APP_ID).build();
    application.connect_activate(build_ui);
    application.run();
}

fn build_ui(app: &Application) {
    install_css();

    let state = demo_state_from_args();
    let body = build_body(&state);
    let status = build_status_card(&state);

    let layout = GtkBox::new(Orientation::Vertical, 8);
    layout.set_halign(gtk::Align::Center);
    layout.append(&body);
    layout.append(&status);
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Lychnos Shell Prototype")
        .child(&layout)
        .decorated(false)
        .resizable(false)
        .build();

    window.init_layer_shell();
    window.set_namespace(Some("lychnos-prototype"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_exclusive_zone(0);
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Right, true);
    window.set_margin(Edge::Top, 36);
    window.set_margin(Edge::Right, 42);

    window.present();
}

fn build_body(state: &CompanionPresentationState) -> DrawingArea {
    let area = DrawingArea::new();
    area.set_content_width(188);
    area.set_content_height(188);
    area.set_tooltip_text(Some("Lychnos prototype — read-only projected state"));

    let mode = state.runtime_mode;
    let approvals = state.pending_approval_count();
    area.set_draw_func(move |_, cr, width, height| {
        let w = f64::from(width);
        let h = f64::from(height);
        let cx = w / 2.0;
        let cy = h / 2.0 - 4.0;
        let radius = w.min(h) * 0.34;

        // Soft floating shadow.
        cr.set_source_rgba(0.0, 0.72, 0.92, 0.16);
        cr.arc(
            cx,
            cy + radius + 18.0,
            radius * 0.72,
            0.0,
            std::f64::consts::TAU,
        );
        let _ = cr.fill();

        // Glossy black shell.
        cr.set_source_rgb(0.025, 0.035, 0.045);
        cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();

        cr.set_line_width(3.0);
        cr.set_source_rgba(0.16, 0.23, 0.28, 0.95);
        cr.arc(cx, cy, radius - 3.0, 0.0, std::f64::consts::TAU);
        let _ = cr.stroke();

        // Segmented shell seams.
        cr.set_line_width(1.4);
        cr.set_source_rgba(0.20, 0.33, 0.39, 0.55);
        for angle in [0.55_f64, 1.75, 2.95, 4.15, 5.35] {
            let x1 = cx + angle.cos() * radius * 0.72;
            let y1 = cy + angle.sin() * radius * 0.72;
            let x2 = cx + angle.cos() * radius * 0.96;
            let y2 = cy + angle.sin() * radius * 0.96;
            cr.move_to(x1, y1);
            cr.line_to(x2, y2);
            let _ = cr.stroke();
        }

        let (r, g, b) = mode_accent(mode);

        // Face panel.
        cr.set_source_rgba(0.015, 0.08, 0.10, 0.96);
        rounded_rect(cr, cx - 39.0, cy - 25.0, 78.0, 50.0, 18.0);
        let _ = cr.fill();

        cr.set_source_rgba(r, g, b, 0.22);
        rounded_rect(cr, cx - 39.0, cy - 25.0, 78.0, 50.0, 18.0);
        cr.set_line_width(2.0);
        let _ = cr.stroke();

        // Expressive cyan eyes.
        cr.set_source_rgb(r, g, b);
        cr.set_line_width(5.0);
        cr.set_line_cap(gtk::cairo::LineCap::Round);
        let eye_y = cy - 2.0;
        cr.move_to(cx - 24.0, eye_y);
        cr.line_to(cx - 10.0, eye_y + eye_tilt(mode));
        let _ = cr.stroke();

        cr.move_to(cx + 10.0, eye_y + eye_tilt(mode));
        cr.line_to(cx + 24.0, eye_y);
        let _ = cr.stroke();

        // Approval notification pip.
        if approvals > 0 {
            cr.set_source_rgb(1.0, 0.55, 0.18);
            cr.arc(
                cx + radius * 0.72,
                cy - radius * 0.72,
                8.0,
                0.0,
                std::f64::consts::TAU,
            );
            let _ = cr.fill();
        }
    });

    area
}

fn build_status_card(state: &CompanionPresentationState) -> GtkBox {
    let card = GtkBox::new(Orientation::Vertical, 2);
    card.add_css_class("status-card");

    let title = Label::new(Some("LYCHNOS"));
    title.add_css_class("lychnos-title");

    let mode = Label::new(Some(mode_label(state.runtime_mode)));
    mode.add_css_class("mode-label");

    let detail = if state.pending_approval_count() > 0 {
        format!("{} approval waiting", state.pending_approval_count())
    } else if let Some(diagnostic) = &state.latest_diagnostic {
        diagnostic.message.clone()
    } else {
        "Standing by".to_string()
    };

    let detail_label = Label::new(Some(&detail));
    detail_label.add_css_class("detail-label");
    detail_label.set_max_width_chars(34);
    detail_label.set_wrap(true);

    card.append(&title);
    card.append(&mode);
    card.append(&detail_label);
    card
}

fn demo_state_from_args() -> CompanionPresentationState {
    let requested = std::env::var("LYCHNOS_DEMO_STATE").unwrap_or_else(|_| "normal".into());

    let runtime_mode = match requested.as_str() {
        "game" | "gamemode" => RuntimeMode::GameMode,
        "disabled" => RuntimeMode::Disabled,
        _ => RuntimeMode::Normal,
    };

    let pending_approvals = if requested == "approval" {
        vec![PendingApprovalPresentation {
            action_id: lychnos_core::action::ActionId::new("demo-approval"),
            kind: lychnos_core::action::ActionKind::new("prototype.preview"),
            capability: lychnos_core::action::Capability::new("prototype.preview"),
            impact: lychnos_core::action::ActionImpact::StateChanging,
            risk: lychnos_core::action::ActionRisk::Low,
            reason: "Prototype approval indicator".into(),
        }]
    } else {
        Vec::new()
    };

    let latest_diagnostic = (requested == "alert").then(|| DiagnosticPresentation {
        level: DiagnosticLevel::Warning,
        component: "prototype".into(),
        message: "Terminal 3 hit a snag…".into(),
    });

    CompanionPresentationState {
        runtime_mode,
        diagnostics_enabled: true,
        pending_approvals,
        tracked_work: Vec::new(),
        latest_diagnostic,
    }
}

fn mode_accent(mode: RuntimeMode) -> (f64, f64, f64) {
    match mode {
        RuntimeMode::Normal => (0.05, 0.86, 1.0),
        RuntimeMode::GameMode => (0.55, 0.38, 1.0),
        RuntimeMode::Disabled => (0.43, 0.48, 0.52),
    }
}

fn mode_label(mode: RuntimeMode) -> &'static str {
    match mode {
        RuntimeMode::Normal => "NORMAL · AWAKE",
        RuntimeMode::GameMode => "GAME MODE · QUIET",
        RuntimeMode::Disabled => "DISABLED · SAFE",
    }
}

fn eye_tilt(mode: RuntimeMode) -> f64 {
    match mode {
        RuntimeMode::Normal => -1.0,
        RuntimeMode::GameMode => 2.5,
        RuntimeMode::Disabled => 0.0,
    }
}

fn rounded_rect(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    let pi = std::f64::consts::PI;
    cr.new_sub_path();
    cr.arc(x + w - radius, y + radius, radius, -pi / 2.0, 0.0);
    cr.arc(x + w - radius, y + h - radius, radius, 0.0, pi / 2.0);
    cr.arc(x + radius, y + h - radius, radius, pi / 2.0, pi);
    cr.arc(x + radius, y + radius, radius, pi, 3.0 * pi / 2.0);
    cr.close_path();
}

fn install_css() {
    let css = gtk::CssProvider::new();
    css.load_from_data(
        r#"
        window { background: transparent; }
        .status-card {
            background: rgba(8, 15, 20, 0.88);
            border: 1px solid rgba(51, 210, 255, 0.26);
            border-radius: 14px;
            padding: 8px 14px;
        }
        .lychnos-title {
            color: #80e9ff;
            font-size: 10px;
            font-weight: 800;
            letter-spacing: 2px;
        }
        .mode-label {
            color: #e8fbff;
            font-size: 11px;
            font-weight: 700;
        }
        .detail-label {
            color: rgba(225, 244, 248, 0.74);
            font-size: 10px;
        }
        "#,
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
