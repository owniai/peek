// core.kt — all kind classifications, scope paths, signature formats, nested scope
// Each construct appears once; no duplicate coverage across core and edge.

package com.example.core

// ── Function ──
fun topFunc(x: Int): String = x.toString()

// ── Const ──
const val APP_NAME = "MyApp"
const val APP_VERSION = "1.0.0"
const val MAX_RETRIES = 3

// ── Var ──
var globalCounter = 0

// ── Alias ──
typealias StringMap = Map<String, String>

// ── Class ──
class MyClass(val name: String) {
    // ── Method ──
    fun regularMethod(): String = name

    // ── Property ──
    var displayName: String = ""
    val isActive: Boolean = true
}

// ── Interface ──
interface Drawable {
    fun draw()
}

// ── Enum ──
enum class Color {
    RED
}

// ── Annotation ──
annotation class Injectable

// ── Object ──
object MathUtils {
    fun square(n: Int): Int = n * n
}

// ── Operator ──
class Vec {
    operator fun plus(other: Vec): Vec = this
}

// ── Getter ──
class Product {
    val name: String
        get() = "product"
}

// ── Setter ──
class Timer {
    var timeout: Int = 0
        set(value) { field = value }
}

// ── Deep nested scope ──
class L1 {
    class L2 {
        class L3 {
            fun deepMethod() {}
        }
    }
}

// ── Same-name in different scopes ──
class Alpha {
    fun process() {}
}

class Beta {
    fun process() {}
}

// ── Property in value category ──
class UserProfile {
    var displayName: String = ""
    val isActive: Boolean = true
}