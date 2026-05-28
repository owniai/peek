// edge.cs — boundary behaviors: record_struct → Struct, indexer (Subscript),
// operator overloading, destructor, multi-const declarator, file-scoped namespace,
// function-body NOT extracted, readonly property getter, multi-line flattening

namespace MyApp.Edge;

// ── Record struct → Struct (not Record) ──
public record struct Coord(double X, double Y);

// ── Record (reference type) → Record kind ──
public record Mailbox(string Address, string DisplayName);

// ── Indexer → Subscript with getter/setter ──
public class MyList
{
    public string this[int index]
    {
        get { return ""; }
        set { }
    }
}

// ── Operator overloading ──
public class Vector
{
    public static Vector operator +(Vector a, Vector b) { return a; }
}

// ── Destructor ──
public class Resource
{
    ~Resource() { }
}

// ── Multi-const declarator extraction ──
public static class Config
{
    public const int PortA = 80, PortB = 443;
}

// ── Readonly property getter ──
public class Settings
{
    public string Name { get; }
}

// ── Field vs Const distinction ──
public class Product
{
    public int Id;
    private string _name;
    protected double Price;
}

public struct Coordinate
{
    public double Latitude;
    public double Longitude;
}

// ── Method vs MethodDeclaration distinction ──
public class EmailService
{
    public void Send(string to, string body) { }
}

public interface IEmailSender
{
    void Send(EmailMessage message);
}

// ── Readonly struct with getter ──
public readonly struct EmailMessage
{
    public string To { get; }
    public string Body { get; }
}

// ── Function-body NOT extracted ──
public class Factory
{
    public void Build()
    {
        var local = 42;
    }
}

// ── Multi-line flattening ──
public class Processor
{
    public void Analyze(
        string input,
        int threshold) { }
}

// ── Record class (explicit reference type) → Record kind ──
public record class EmailTemplate(string Subject, string Body);