// Swift comprehensive test fixture for peek
// Tests all 9 definition types: class, struct, enum, protocol, actor,
// extension, function, typealias, const

// === Top-level definitions ===

class NetworkManager {
    let maxConnections: Int
    var activeConnections: Int

    static let shared = NetworkManager()

    init() {
        self.maxConnections = 10
        self.activeConnections = 0
    }

    func connect(host: String) -> Bool {
        return true
    }

    static func create() -> NetworkManager {
        return NetworkManager()
    }

    struct Config {
        let timeout: Double
        let retryCount: Int
    }

    enum Status {
        case connected
        case disconnected
        case error

        func describe() -> String {
            return ""
        }
    }
}

struct Point {
    var x: Double
    var y: Double

    func distance(to other: Point) -> Double {
        return 0.0
    }
}

enum Direction {
    case north
    case south
    case east
    case west
}

protocol Serializable {
    func serialize() -> Data
    var serializedSize: Int { get }
}

actor MessageQueue {
    var messages: [String] = []

    func enqueue(_ message: String) -> Int {
        messages.append(message)
        return messages.count
    }

    func dequeue() -> String? {
        return messages.first
    }
}

extension String {
    let defaultEncoding = "UTF-8"

    var trimmed: String {
        return self
    }

    func repeated(_ times: Int) -> String {
        return self
    }
}

typealias CompletionHandler = (Bool) -> Void

let MAX_RETRIES = 3

func processRequest(_ url: String) -> Bool {
    return true
}
