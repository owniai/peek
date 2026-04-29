<?php
namespace App\Services {
    class UserService
    {
        public function find(int $id): ?User
        {
            return null;
        }
    }
}

namespace App\Models {
    class User
    {
        public string $name;
    }
}

namespace {
    function helper(): void {}
}
