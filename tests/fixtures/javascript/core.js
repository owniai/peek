// core.js — all kind classifications, scope paths, signature formats
// Each construct appears once; no duplicate coverage across core and edge.

// ── Function ──
function topFunc(x, y) {
  return x + y;
}

async function asyncFetch(url) {
  return await fetch(url);
}

// ── Class ──
class MyClass extends Base {
  // ── Constructor ──
  constructor(name) {
    this.name = name;
  }

  // ── Method ──
  regularMethod() {
    return 1;
  }

  static staticHelper() {
    return 2;
  }

  // ── Getter ──
  get fullName() {
    return this.name;
  }

  // ── Setter ──
  set fullName(value) {
    this.name = value;
  }

  // ── Field ──
  pubField = 1;
}

// ── Const ──
const API_KEY = 'secret';

// ── Var (let) ──
let counter = 0;

// ── Var (var keyword) ──
var legacyVar = true;

// ── Arrow function as const → Function ──
const handleClick = (e) => e.target;

// ── Export keyword in signature ──
export function exportedFunc() {
  return 1;
}

export class ExportedClass {
}

export const EXPORTED_CONST = 42;

// ── Same-name in different scopes ──
function process() {}

class Alpha {
  process() {}
}

class Beta {
  process() {}
}

// ── Multiline signature ──
function multiline(
  x,
  y,
  z
) {
  return x + y + z;
}