<?php
// Comprehensive PHP fixture — covers all 6 definition types + namespace scopes + attributes

// === Simple namespace syntax ===
namespace App\Models;

class User
{
    public string $name;
    public int $age;

    const MIN_AGE = 18;

    public function getName(): string
    {
        return $this->name;
    }
}

interface Renderable
{
    public function render(): string;
}

trait Loggable
{
    public function log(string $message): void
    {
        echo $message;
    }
}

enum Status
{
    case Active;
    case Inactive;
}

enum Color: string
{
    case Red = 'red';
    case Blue = 'blue';
}

function helper(): void
{
}

const APP_VERSION = '1.0.0';

// === Brace namespace syntax ===
namespace App\Services {

    class UserService
    {
        public function find(int $id): ?User
        {
            return null;
        }
    }

    class EmailService
    {
        public function send(string $to, string $body): void
        {
        }
    }
}

namespace App\Config {
    const MAX_RETRIES = 3;
    const DB_HOST = 'localhost';

    class Database
    {
        public function connect(): void
        {
        }
    }
}

// === Global namespace ===
namespace {
    function global_func(): void
    {
    }

    const GLOBAL_CONST = 42;
}

// === Attributes ===
namespace App\Attributes;

#[\Attribute]
class Route
{
    public function __construct(public string $path) {}
}

#[Route("/api/users")]
class UserController
{
    #[Inject]
    public function handle(): void
    {
    }
}

// === Multi-variable const ===
namespace App\Settings;

const DEBUG = true, CACHE_TTL = 3600, MAX_ITEMS = 100;
