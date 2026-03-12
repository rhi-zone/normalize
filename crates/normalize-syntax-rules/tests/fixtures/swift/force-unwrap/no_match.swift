func foo() {
    guard let name = user.name else { return }
    let other = user.email ?? "unknown"
}
