use proptest::prelude::*;

use crate::{INKS, NEAR, Refusal, compile, ink};

#[derive(Clone, Debug)]
enum Title {
    Missing,
    Blank(String),
    Given(String),
}

#[derive(Clone, Debug)]
struct Shape {
    fill: Option<[u8; 3]>,
    stroke: Option<[u8; 3]>,
    at: (u8, u8),
}

fn color() -> impl Strategy<Value = [u8; 3]> {
    prop_oneof![
        prop::sample::select(INKS.to_vec()).prop_map(|(_, rgb)| rgb),
        (prop::sample::select(INKS.to_vec()), -9i16..=9, -9i16..=9, -9i16..=9).prop_map(|((_, [r, g, b]), dr, dg, db)| {
            let near = |c: u8, d: i16| (c as i16 + d).clamp(0, 255) as u8;
            [near(r, dr), near(g, dg), near(b, db)]
        }),
        any::<[u8; 3]>(),
    ]
}

fn shape() -> impl Strategy<Value = Shape> {
    (prop::option::of(color()), prop::option::of(color()), (0u8..12, 0u8..12)).prop_map(|(fill, stroke, at)| Shape { fill, stroke, at })
}

fn title() -> impl Strategy<Value = Title> {
    prop_oneof![Just(Title::Missing), "[ \t\n]{0,3}".prop_map(Title::Blank), "[A-Za-z][A-Za-z ]{0,12}|長い絵|שלום".prop_map(Title::Given),]
}

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn svg(title: &Title, shapes: &[Shape]) -> String {
    let mut out = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16">"#);
    match title {
        Title::Missing => {}
        Title::Blank(t) | Title::Given(t) => out.push_str(&format!("<title>{t}</title>")),
    }
    for s in shapes {
        let fill = s.fill.map_or("none".to_string(), hex);
        let stroke = s.stroke.map_or("none".to_string(), hex);
        out.push_str(&format!(r#"<rect x="{}" y="{}" width="4" height="4" fill="{fill}" stroke="{stroke}"/>"#, s.at.0, s.at.1));
    }
    out.push_str("</svg>");
    out
}

fn near_an_ink(c: [u8; 3]) -> bool {
    INKS.iter().any(|(_, rgb)| c.iter().zip(rgb).map(|(a, b)| (*a as f32 - *b as f32).powi(2)).sum::<f32>().sqrt() <= NEAR)
}

proptest! {
    #[test]
    fn a_picture_without_a_label_is_refused(title in title(), shapes in prop::collection::vec(shape(), 0..5)) {
        let shapes: Vec<_> = shapes.into_iter().map(|s| Shape { fill: s.fill.map(|_| INKS[0].1), stroke: s.stroke.map(|_| INKS[0].1), ..s }).collect();
        let compiled = compile("p", &svg(&title, &shapes));
        match title {
            Title::Given(t) => prop_assert_eq!(compiled.map(|p| p.label), Ok(t.split_whitespace().collect::<Vec<_>>().join(" "))),
            Title::Missing | Title::Blank(_) => prop_assert_eq!(compiled.map(|p| p.label), Err(Refusal::NoLabel)),
        }
    }

    #[test]
    fn every_color_lands_on_a_semantic_ink(shapes in prop::collection::vec(shape(), 1..5)) {
        let used: Vec<[u8; 3]> = shapes.iter().flat_map(|s| [s.fill, s.stroke]).flatten().collect();
        match compile("p", &svg(&Title::Given("Shapes".into()), &shapes)) {
            Ok(p) => {
                prop_assert!(used.iter().all(|c| near_an_ink(*c)), "{:?} were let through", used);
                let inks: Vec<_> = used.iter().map(|c| ink(*c).expect("near an ink")).collect();
                prop_assert_eq!(p.layers.len(), used.len());
                for l in &p.layers {
                    prop_assert!(inks.contains(&l.ink), "{} is not one of {:?}", l.ink, inks);
                }
            }
            Err(Refusal::NotInk { color, .. }) => {
                prop_assert!(used.contains(&color));
                prop_assert!(!near_an_ink(color));
            }
            Err(other) => prop_assert!(false, "refused for {other}"),
        }
    }
}
