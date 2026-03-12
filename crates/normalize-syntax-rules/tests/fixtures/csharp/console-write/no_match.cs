using Microsoft.Extensions.Logging;
class Foo {
    private readonly ILogger _logger;
    void Bar() {
        _logger.LogInformation("User logged in");
    }
}
