<?php
// edge.php — boundary behaviors: define() as Const, promoted constructor property,
// multi-const declaration, namespace semicolon vs brace scope, mixed HTML,
// function-body NOT extracted, attributes

// ── Namespace with semicolon ──
namespace App\Config;

// ── Multi-const declaration ──
const DEBUG = true, CACHE_TTL = 3600, MAX_ITEMS = 100;

// ── Namespace with brace syntax ──
namespace App\Services {
    class UserService
    {
        // ── Promoted constructor property as Property ──
        public function __construct(private int $id) {}
    }
}

// ── Attributes namespace with promoted properties ──
namespace App\Attributes {
    #[\Attribute]
    class Route
    {
        // ── Promoted constructor property (public) as Property ──
        public function __construct(public string $path) {}
    }

    #[Route("/api/users")]
    class UserController
    {
        // ── Promoted constructor property (private) as Property ──
        public function __construct(private int $id) {}
    }
}

// ── Global namespace (brace, no name) ──
namespace {
    // ── define() as Const ──
    define('GLOBAL_DEFINE', 'value');

    // ── Function-body NOT extracted ──
    function outer(): void
    {
        define('BODY_DEFINE', 'should_not_appear');
    }
}

// ── Mixed HTML ──
?>
<p>HTML content</p>
<?php
class Page
{
    public function render(): string {}
}
?>