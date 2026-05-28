// edge.ts — boundary behaviors: declaration vs definition, function-body exclusion,
// satisfies/as expressions, TSX grammar

// ── declare function → FunctionDeclaration ──
declare function declaredFunc(x: number): void;

// ── Declaration interface: all declaration variant kinds ──
interface DeclarationInterface {
    declaredMethod(y: string): number;
    get declaredGetter(): string;
    set declaredSetter(v: string);
    declaredProperty: boolean;
    [key: string]: unknown;
    new(x: number): DeclarationInterface;
    (input: string): void;
}

// ── declare const/var/let ──
declare const declaredConst: string;
declare let declaredLet: number;
declare var declaredVar: boolean;

// ── declare abstract class → ClassDeclaration ──
declare abstract class DeclaredAbstractClass {
    abstract doStuff(): void;
}

// ── declare enum → EnumDeclaration ──
declare enum DeclaredEnum { A }

// ── function-body definitions NOT extracted ──
function factory() {
    const localConst = 1;
    function innerFunc() {}
    interface LocalInterface {}
    type LocalType = string;
    enum LocalEnum { A, B }
    let localVar = 2;
}

// ── satisfies expression ──
const satisfiesConfig = {
    init() {},
    destroy() {}
} satisfies Record<string, Function>;

// ── as expression ──
const asHandler = ((msg: string) => msg.toUpperCase()) as (s: string) => string;

// ── satisfies with class ──
const satisfiesService = class {
    process() {}
} satisfies { process(): void };

// ── TSX grammar ──
function TsxComponent() { return null; }
const TsxArrow = () => null;