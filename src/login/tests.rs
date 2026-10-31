use super::*;
#[test]
fn login_screen_has_manual_start_and_full_public_url_without_the_private_verifier() {
    let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
    let mut m = Model::default();
    t.draw(|f| {
        view::draw(f, &m);
    })
    .unwrap();
    let initial = t
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(initial.contains("Sign in with browser"));
    let token = "a".repeat(100);
    let url = format!("https://lume.dpdns.org/login?code_token={token}");
    m.phase = Phase::Waiting;
    m.link = Some(flow::Link {
        token: token.clone(),
        url,
        code: "aaaa-aaaa".into(),
        expires_in: 600,
    });
    m.link_started = Some(Instant::now());
    t.draw(|f| {
        view::draw(f, &m);
    })
    .unwrap();
    let text = t
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("Public sign-in token"));
    assert!(text.contains("Copy URL [C]"));
    assert!(text.contains("code_token="));
    assert!(!text.contains("poll_secret"));
    assert!(text.matches('a').count() >= 200);
}
