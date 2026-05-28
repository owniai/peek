<?php
// core.php — all kind classifications, scope paths, signature formats
// Each construct appears once; no duplicate coverage across core and edge.

namespace App\Models;

// ── Function ──
function helper(): void {}

// ── Class ──
class User
{
    // ── Property ──
    public string $name;

    // ── Const ──
    const MIN_AGE = 18;

    // ── Constructor ──
    public function __construct(string $name) {}

    // ── Destructor ──
    public function __destruct() {}

    // ── Getter (__get) ──
    public function __get(string $key): mixed {}

    // ── Setter (__set) ──
    public function __set(string $key, mixed $val): void {}

    // ── Operator (__invoke) ──
    public function __invoke(): int {}

    // ── Method ──
    public function getName(): string {}
}

// ── Interface ──
interface Renderable
{
    public function render(): string;
}

// ── Trait ──
trait Loggable
{
    public function log(string $msg): void {}
}

// ── Enum ──
enum Status
{
    case Active;
}

// ── Same-name in different scopes ──
class Alpha
{
    public function process(): void {}
}

class Beta
{
    public function process(): void {}
}