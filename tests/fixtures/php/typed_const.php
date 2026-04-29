<?php
// PHP 8.3+ typed class constants

namespace App\Config;

class Settings {
    // Typed constants (PHP 8.3+)
    const string APP_NAME = 'peek';
    const int MAX_RETRIES = 3;
    const bool DEBUG_MODE = false;

    // Untyped constants (traditional)
    const VERSION = '1.0.0';

    // Multiple typed constants in one declaration
    const string DB_HOST = 'localhost';
    const string DB_NAME = 'peek_db';

    // Visibility modifier + typed constant
    public const string PUBLIC_CONST = 'public';
    protected const string PROTECTED_CONST = 'protected';
    private const string PRIVATE_CONST = 'private';
}

enum Status: string {
    case Active = 'active';

    const string DEFAULT = 'active';
}
