func foo() async throws {
    try await Task.sleep(nanoseconds: 1_000_000_000)
}
