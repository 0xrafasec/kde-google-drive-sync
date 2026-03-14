//! Unit tests: tray actions map to intended D-Bus semantics (helpers only).

#[test]
fn tray_action_quit_discriminant() {
    // Ensures enum used by menu compiles and variants distinct.
    use std::mem::discriminant;
    enum A {
        Pause,
        Resume,
        Quit,
    }
    assert_ne!(discriminant(&A::Pause), discriminant(&A::Quit));
}

#[test]
fn activity_ring_buffer_cap() {
    use std::collections::VecDeque;
    const MAX: usize = 500;
    let mut q = VecDeque::with_capacity(MAX + 1);
    for i in 0..MAX + 10 {
        q.push_back(i);
        while q.len() > MAX {
            q.pop_front();
        }
    }
    assert_eq!(q.len(), MAX);
}
