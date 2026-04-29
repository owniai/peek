// Fixture: out-of-class method definitions
// Tests that C++ parser can find method definitions outside the class body

class Engine {
public:
    void start();
    void stop();
};

// Out-of-class method definitions
void Engine::start() {
    // implementation
}

void Engine::stop() {
    // implementation
}

// Also test: function returning const type (const function prototype)
const int compute();
