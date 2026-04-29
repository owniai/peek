// Package sample provides comprehensive Go AST test fixtures.
package sample

import "fmt"

// --- Top-level functions ---

func simpleFunc() {}

func withParams(x int, y string) {}

func withReturn() int {
	return 42
}

func withMultipleReturn() (int, error) {
	return 0, nil
}

func withNamedReturn() (result int, err error) {
	return
}

func variadicFunc(items ...string) {}

// --- Methods (receivers) ---

type Server struct {
	Host string
	Port int
}

func (s Server) ValueReceiverMethod() string {
	return s.Host
}

func (s *Server) PointerReceiverMethod() error {
	return nil
}

func (s *Server) MethodWithParams(timeout int) (bool, error) {
	return true, nil
}

// --- Struct types ---

type Point struct {
	X float64
	Y float64
}

type Embedded struct {
	Server      // embedded struct
	Name string
}

type Tagged struct {
	Field string `json:"field" yaml:"field"`
}

// --- Interface types ---

type Reader interface {
	Read(p []byte) (n int, err error)
}

type Writer interface {
	Write(p []byte) (n int, err error)
}

type ReadWriter interface {
	Reader
	Writer
}

type EmptyInterface interface{}

// --- Type aliases (Go 1.9+) ---

type AliasInt = int
type AliasMap = map[string]int

// --- Type definitions ---

type DefinedInt int
type DefinedSlice []string

// --- Constants ---

const MaxSize = 1024

const (
	StatusOK    = 200
	StatusError = 500
)

const (
	Red   = iota
	Green
	Blue
)

const TypedConst string = "hello"

// --- Variables ---

var GlobalVar int = 42

var (
	VarA = "a"
	VarB = 1
)

// --- Generic functions and types (Go 1.18+) ---

func GenericFunc[T any](value T) T {
	return value
}

func GenericMulti[T any, U any](t T, u U) (T, U) {
	return t, u
}

type Container[T any] struct {
	Value T
}

func (c *Container[T]) Get() T {
	return c.Value
}

func (c *Container[T]) Set(value T) {
	c.Value = value
}

type Pair[K comparable, V any] struct {
	Key   K
	Value V
}

// --- Interface with generic method ---

type Transformer[From any, To any] interface {
	Transform(from From) To
}

// --- Type switch and other patterns ---

func typeSwitch(x interface{}) {
	switch v := x.(type) {
	case int:
		fmt.Println(v)
	case string:
		fmt.Println(v)
	}
}

// --- Function with closure ---

func withClosure() func(int) int {
	sum := 0
	return func(x int) int {
		sum += x
		return sum
	}
}

// --- Method on defined type ---

func (d DefinedInt) IsPositive() bool {
	return d > 0
}

// --- Multiple methods on same type with different receivers ---

type MultiMethod struct {
	data int
}

func (m MultiMethod) ValueMethod() int { return m.data }
func (m *MultiMethod) PtrMethod() int  { return m.data }

// --- Grouped type declarations ---

type (
	GroupedPoint struct {
		X float64
		Y float64
	}
	GroupedHandler interface {
		Handle() error
	}
	GroupedInt int
	GroupedAlias = string
)

// --- Definitions inside function body (should NOT be extracted) ---

func withLocalDefs() {
	const LocalConst = 42
	type LocalType struct{ x int }
}
