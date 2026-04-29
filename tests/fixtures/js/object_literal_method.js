// Object literal methods should NOT be extracted as top-level definitions.
// This is a bug repro file: method_definition inside an object literal is
// incorrectly extracted as a Function definition.

const config = {
  init() {
    return 1;
  },
  destroy() {
    return 2;
  }
};
