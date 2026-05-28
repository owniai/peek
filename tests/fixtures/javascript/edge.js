// edge.js — boundary behaviors: function-body exclusion, object literal pair
// extraction, private fields, anonymous arrow, export default, var/let/const

// ── Function-body definitions NOT extracted ──
function factory() {
  function innerFunc() {}
  class InnerClass {}
  const innerConst = 1;
  let innerLet = 2;
  var innerVar = 3;
}

// ── Object literal pair extraction ──
// pair with method_definition → Method
// pair with function_expression → Method
// pair with arrow_function → Method
// pair with string value → Field
// pair with generator → Method
// pair with class value → Class
const config = {
  init() { return 1; },
  handler: function() { return 2; },
  cb: () => 3,
  name: "hello",
  gen: function*() { yield 1; },
  Klass: class { run() {} },
};

// ── Private field (# prefix) ──
class PrivateFields {
  #privField = 2;
  pubField2 = 3;
}

// ── Anonymous arrow function NOT extracted ──
setTimeout(() => { console.log('hi'); }, 1000);

// ── Export default ──
export default function defaultFunc() {
  return 1;
}

export default class DefaultClass {
}

// ── var vs let vs const distinction ──
const constOnly = 'a';
let letVar = 'b';
var varOnly = 'c';

// ── Object literal inside function body NOT extracted ──
function setup() {
  const localObj = { method() {} };
}

// ── Class expression ──
const ExprClass = class {
  exprMethod() {}
};

// ── Computed property key NOT extracted ──
const dynamicObj = { [computedKey]: 42 };

// ── Class field with arrow value → Field (not Function) ──
class ArrowField {
  arrowProp = () => {};
}