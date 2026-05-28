package edge

type S struct {
	data int
}

func (s S) ValRecv() int   { return s.data }
func (s *S) PtrRecv() int  { return s.data }

type Embed struct {
	Name string
	S
}

type PtrEmbed struct {
	*http.Server
	Timeout int
}

const (
	A = 1
	B = 2
)

const TypedConst string = "hello"

const (
	Red   = iota
	Green
	Blue
)

var (
	VarA = "a"
	VarB = 1
)

type (
	GroupedStruct struct{ X int }
	GroupedInt int
)

func outer() {
	const innerConst = 1
	var innerVar int
}

type Container[T any] struct {
	Value T
}

func (c *Container[T]) Get() T { return c.Value }
func (c *Container[T]) Set(value T) { c.Value = value }

type EmptyInterface interface{}

type BaseIO interface {
	Read() error
}

type CombinedIO interface {
	BaseIO
	Write() error
}

func multilineSig(
	x int,
	y string,
) (int, error) {
	return 0, nil
}

type Tagged struct {
	Field string `json:"field" yaml:"field"`
}

type (
	GroupedPoint  struct{ X float64; Y float64 }
	GroupedHandler interface{ Handle() error }
	GroupedAlias   = string
)