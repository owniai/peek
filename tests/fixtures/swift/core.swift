// core.swift — all kind classifications, scope paths, signature formats, nested scope
// Each construct appears once; no duplicate coverage across core and edge.

// ── Function ──
func topFunc(x: Int) -> Bool {
    return true
}

// ── Var (top-level) ──
var globalCounter = 0

// ── Const (top-level let) ──
let MAX_RETRIES = 3

// ── Class ──
class NetworkManager {
    // ── Const (class-level let) ──
    let maxConnections: Int

    // ── Property (class-level var) ──
    var activeConnections: Int

    // ── Constructor ──
    init() {
        self.maxConnections = 10
        self.activeConnections = 0
    }

    // ── Destructor ──
    deinit {
        cleanup()
    }

    static func create() -> NetworkManager {
        return NetworkManager()
    }

    // ── Method ──
    func connect(host: String) -> Bool {
        return true
    }

    // ── Nested Struct ──
    struct Config {
        let timeout: Double
        var retryCount: Int
    }

    // ── Nested Enum ──
    enum Status {
        case connected
        case disconnected

        func describe() -> String {
            return ""
        }
    }

    // ── Subscript ──
    subscript(index: Int) -> Int {
        get { return 0 }
        set {}
    }
}

// ── Struct ──
struct Point {
    var x: Double
    var y: Double

    func distance(to other: Point) -> Double {
        return 0.0
    }
}

// ── Enum ──
enum Direction {
    case north
    case south
    case east
    case west
}

// ── Protocol ──
protocol Serializable {
    associatedtype SerializedData
    func serialize() -> Data
    var serializedSize: Int { get }
}

// ── Actor ──
actor MessageQueue {
    var messages: [String] = []

    func enqueue(_ message: String) -> Int {
        return 0
    }
}

// ── Extension ──
extension String {
    let defaultEncoding = "UTF-8"

    var trimmed: String {
        return self
    }

    func repeated(_ times: Int) -> String {
        return self
    }
}

// ── Subscript (struct) ──
struct Matrix {
    var grid: [[Double]]

    subscript(row: Int, col: Int) -> Double {
        get { grid[row][col] }
        set { grid[row][col] = newValue }
    }
}

// ── Alias ──
typealias CompletionHandler = (Bool) -> Void

// ── Operator (operator function in struct) ──
struct Vec2d {
    var x: Double
    var y: Double

    static func +(lhs: Vec2d, rhs: Vec2d) -> Vec2d {
        Vec2d(x: lhs.x + rhs.x, y: lhs.y + rhs.y)
    }

    static func ==(lhs: Vec2d, rhs: Vec2d) -> Bool {
        lhs.x == rhs.x && lhs.y == rhs.y
    }
}