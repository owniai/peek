package com.example.extensions

// Top-level extension function
fun String.isEmail(): Boolean = this.contains("@")

// Extension function with modifiers
private fun Int.isPositive(): Boolean = this > 0

// Generic extension function
fun <T> List<T>.secondOrNull(): T? = if (size >= 2) this[1] else null

// Extension function with multiple parameters
fun String.repeat(n: Int): String = this.repeat(n)

// Regular function for comparison
fun regularFunction(x: Int): Int = x * 2
