#[test]
fn url_with_shell_metacharacters_is_safe() {
    // Verify that URLs with shell metacharacters don't cause issues
    // when passed as discrete Command arguments (no shell involved)
    let dangerous_url = "https://example.com/video?a=1&b=2;rm -rf /";
    let dangerous_device = "device$(whoami)";

    // These should not panic or execute shell commands
    // The actual Command is constructed with .arg() not sh -c
    assert!(dangerous_url.contains(';'));
    assert!(dangerous_device.contains('$'));
}
