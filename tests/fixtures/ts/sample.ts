// Functions
function simpleFunc(): void {}
function typedParams(x: number, y: string): boolean { return true; }
function genericFunc<T>(x: T): T { return x; }
async function asyncFunc(): Promise<void> {}
function* generatorFunc(): Generator<number> { yield 1; }
export function exportedFunc(): void {}
export default function defaultFunc(): void {}

// Classes
class SimpleClass {}
class ExtendedClass extends SimpleClass {}
class ImplementingClass implements SimpleClass { method(): void {} }
class GenericClass<T> { value: T; }
abstract class AbstractBase {
  abstract doWork(): void;
  concreteMethod(): string { return "hello"; }
}
export class ExportedClass {}
export default class DefaultExportedClass {}

// Methods
class MethodClass {
  constructor(private x: number) {}
  regularMethod(): void {}
  static staticMethod(): void {}
  async asyncMethod(): Promise<void> {}
  *generatorMethod(): Generator<number> { yield 1; }
  get value(): number { return this.x; }
  set value(v: number) { this.x = v; }
}

// Interfaces
interface SimpleInterface {
  name: string;
}
interface ExtendedInterface extends SimpleInterface {
  age: number;
}
interface GenericInterface<T> {
  value: T;
  getValue(): T;
}
export interface ExportedInterface {}

// Type aliases
type SimpleType = string;
type UnionType = "active" | "inactive";
type IntersectionType = { a: number } & { b: string };
type GenericType<T> = { ok: boolean; data: T };
export type ExportedType = { debug: boolean };

// Enums
enum SimpleEnum { A, B, C }
enum StringEnum { Up = "UP", Down = "DOWN" }
const enum ConstEnum { X = 1, Y = 2 }
export enum ExportedEnum { Active, Inactive }

// Constants
const simpleConst: number = 42;
const arrowFunc = (x: number): number => x * 2;
const funcExpr = function(x: number): number { return x; };
export const exportedConst = "hello";
export const exportedArrow = () => {};

// Nested
function outerFunc() {
  interface LocalInterface {}
  type LocalType = string;
  enum LocalEnum { A, B }
  const localConst = 1;
  function innerFunc() {}
}

// Namespace / Module
namespace MyApp {
  export function nsHelper(): void {}
  export class NsClass {}
}
module ModSpace {
  export function modFunc(): void {}
}

// Fields and Properties
class FieldClass {
  publicId: number;
  private _name: string;
  readonly createdAt: Date;
  static instanceCount: number;
}

interface PropertyInterface {
  id: number;
  label: string;
  optional?: boolean;
  readonly immutable: string;
}
