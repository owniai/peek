// === Top-level function declarations ===

function simpleFunc() {
  return 1;
}

function withParams(x, y) {
  return x + y;
}

function withReturn(x) {
  return x * 2;
}

function withDefault(a, b = 10) {
  return a + b;
}

function withRest(...args) {
  return args;
}

function withDestructure({ name, age }, [x, y]) {
  return name;
}

// === Generator functions ===

function* genFunc() {
  yield 1;
}

// === Async functions ===

async function asyncFunc() {
  return await fetch('/');
}

// === Class declarations ===

class SimpleClass {
}

class WithExtends extends Base {
}

class WithConstructor {
  constructor(name) {
    this.name = name;
  }
}

class WithMethods {
  regularMethod() {
    return 1;
  }

  *generatorMethod() {
    yield 1;
  }

  async asyncMethod() {
    return await fetch('/');
  }

  static staticMethod() {
    return 2;
  }

  get fullName() {
    return this.name;
  }

  set fullName(value) {
    this.name = value;
  }

  arrowField = () => {
    return 3;
  };
}

// === Variable declarations ===

const constVar = 42;
let letVar = 'hello';
var varVar = true;

// === Arrow function as variable ===

const arrowFunc = (x) => x * 2;
const arrowFuncBlock = (x) => {
  return x * 2;
};
const arrowFuncNoParens = x => x + 1;

// === Function expressions ===

const funcExpr = function(x) {
  return x;
};

const namedFuncExpr = function namedFn(x) {
  return x;
};

// === Exported definitions ===

export function exportedFunc() {
  return 1;
}

export class ExportedClass {
}

export const exportedConst = 42;

export default function defaultFunc() {
  return 1;
}

export default class DefaultClass {
}

// === Nested definitions ===

function outerFunc() {
  function innerFunc() {
    return 1;
  }

  class InnerClass {
  }

  const innerArrow = () => 2;

  return innerFunc();
}

class OuterClass {
  innerMethod() {
    function nestedFunc() {
      return 3;
    }
    return nestedFunc();
  }
}

// === Object literal methods (extracted as Function with scope = variableName.methodName) ===

const obj = {
  methodOne() { return 1; },
  methodTwo: function() { return 2; },
  methodThree: () => 3,
};
