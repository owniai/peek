package core

import "fmt"

// --- Top-level functions (from sample.go) ---

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

// --- Generic functions (from sample.go) ---

func GenericFunc[T any](value T) T {
	return value
}

func GenericMulti[T any, U any](t T, u U) (T, U) {
	return t, u
}

// --- Function patterns (from sample.go) ---

func typeSwitch(x interface{}) {
	switch v := x.(type) {
	case int:
		fmt.Println(v)
	case string:
		fmt.Println(v)
	}
}

func withClosure() func(int) int {
	sum := 0
	return func(x int) int {
		sum += x
		return sum
	}
}

func withLocalDefs() {
	const LocalConst = 42
	type LocalType struct{ x int }
}

// --- Struct types ---

type Server struct {
	Host string
	Port int
}

type Point struct {
	X float64
	Y float64
}

type Embedded struct {
	Server      // embedded struct
	Name string
}

type MultiMethod struct {
	data int
}

func (m MultiMethod) ValueMethod() int { return m.data }
func (m *MultiMethod) PtrMethod() int  { return m.data }

// --- Core patterns (original core.go) ---

type Config struct {
	Host string
}

func (c Config) Validate() bool { return true }

func (c *Config) Invalidate() {}

type Alpha struct{}

func (a Alpha) process() {}

type Beta struct{}

func (b Beta) process() {}

// --- Methods on Server (from sample.go) ---

func (s Server) ValueReceiverMethod() string {
	return s.Host
}

func (s *Server) PointerReceiverMethod() error {
	return nil
}

func (s *Server) MethodWithParams(timeout int) (bool, error) {
	return true, nil
}

// --- Interface types (from sample.go) ---

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

// --- Core interface (original core.go) ---

type Storer interface {
	Store(data []byte) error
}

// --- Generic interface (from sample.go) ---

type Transformer[From any, To any] interface {
	Transform(from From) To
}

// --- Generic struct (from sample.go) ---

type Pair[K comparable, V any] struct {
	Key   K
	Value V
}

// --- Generic function (original core.go) ---

func Lookup[T any](key string) (T, error) { var v T; return v, nil }

// --- Type aliases (from sample.go) ---

type AliasInt = int
type AliasMap = map[string]int

// --- Type alias (original core.go) ---

type AliasStr = string

// --- Type definitions (from sample.go) ---

type DefinedSlice []string

// --- Type definition (original core.go) ---

type DefinedInt int

func (d DefinedInt) IsPositive() bool { return d > 0 }

// --- Constants (from sample.go) ---

const MaxSize = 1024

const (
	StatusOK    = 200
	StatusError = 500
)

// --- Constant (original core.go) ---

const MaxLimit = 100

// --- Variables (from sample.go) ---

var GlobalVar int = 42

// --- Variable (original core.go) ---

var DebugMode bool