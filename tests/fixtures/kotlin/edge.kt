// edge.kt — boundary behaviors: operator not Method, destructuring extraction,
// class parameter properties, companion object const, sealed nesting,
// enum entry body, getter+setter together, secondary constructor,
// function-body NOT extracted

// ── Operator fun classified as Operator, NOT Method ──
class Token(val value: String) {
    operator fun contains(char: Char): Boolean = value.contains(char)
}

// ── Destructuring val/var extraction ──
val (destX, destY) = getPoint()
var (destA, destB) = getCoords()

class DataHolder {
    val (holderName, holderAge) = parseData()
    var (holderScore, holderLevel) = loadStats()
}

// ── Class parameter properties ──
class MixedParams(
    val propParam: String,
    var mutableParam: Int,
    plainParam: Double
)

// ── Companion object const ──
class Config {
    companion object Defaults {
        const val DEFAULT_TIMEOUT = 30
    }
}

class Repository {
    companion object {
        const val MAX_RESULTS = 100
    }
}

// ── Sealed class/interface nesting ──
sealed class Shape {
    class Circle : Shape()
}

sealed interface Node {
    class Literal : Node
}

// ── Enum entry body methods/properties ──
enum class Planet(val mass: Double) {
    EARTH(5.97e24) {
        override fun toString(): String = "Earth"
    },
    MARS(6.42e23) {
        val gravity = 3.7
        fun describe(): String = "Mars"
    };
}

// ── Getter + Setter together on same property ──
class Widget {
    var name: String = ""
        get() = field.trim()
        set(value) { field = value.lowercase() }
}

// ── Secondary constructor ──
class Person(val firstName: String, val lastName: String) {
    constructor(fullName: String) : this(fullName.split(" ")[0], fullName.split(" ")[1])
}

// ── Function-body NOT extracted ──
fun factory() {
    val localVal = 1
    var localVar = 2
    fun inner() {}
}