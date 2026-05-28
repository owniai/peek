// core.ts — all kind classifications, scope paths, signature formats, nested scope
// Each construct appears once; no duplicate coverage across core and edge.

// ── Function (export signature) ──
export function topFunc(x: number): string { return ""; }
function simpleFunc(): void {}

// ── Const (export signature) ──
const maxRetries: number = 3;
export const exportedConst = "hello";

// ── Var (export signature) ──
let mutableVar: string = "hello";
export let exportedVar = 42;

// ── Class (abstract signature + export) ──
abstract class MyClass {
    // ── Field ──
    name: string = "default";

    // ── Constructor ──
    constructor(input: string) {}

    // ── Method ──
    regularMethod(): void {}

    // ── Getter ──
    get count(): number { return 0; }

    // ── Setter ──
    set count(value: number) {}
}
export class ExportedClass {}

// ── Class with public field ──
class FieldClass {
    publicId: number;
}

// ── Interface ──
interface MyInterface {
    // ── Property ──
    id: number;

    // ── Subscript ──
    [key: string]: unknown;
}

// ── Alias ──
type Point = { x: number; y: number };

// ── Enum ──
enum Color {
    // ── Variant ──
    Red = 1,
}

// ── Namespace ──
namespace MyApp {
    export function nsHelper(): void {}
    export class NsClass {}
}

// ── Module (module keyword) ──
module ModSpace {
    export function modFunc(): void {}
}

// ── ModuleDeclaration (declare signature) ──
declare module "express" {
    export interface Request {}
}
declare module "./relative-module" {
    export const value: number;
}
declare module "shorthand";

// ── Deep nested scope ──
namespace L1 {
    namespace L2 {
        namespace L3 {
            export function deepFunc(): void {}
        }
    }
}

// ── Same-name in different scopes ──
function process(): void {}

class Alpha {
    process(): void {}
}

class Beta {
    process(): void {}
}