// Comprehensive C# fixture — covers edge cases not in sample.cs

// === File-scoped namespace with multiple type definitions ===
namespace Comprehensive.Services;

using System;

// Class in file-scoped namespace
public class EmailService
{
    public void Send(string to, string body) { }

    public static EmailService CreateDefault()
    {
        return new EmailService();
    }
}

// Struct in file-scoped namespace
public readonly struct EmailMessage
{
    public string To { get; }
    public string Body { get; }
}

// Interface in file-scoped namespace
public interface IEmailSender
{
    void Send(EmailMessage message);
    event EventHandler<string> OnSent;
}

// Delegate in file-scoped namespace
public delegate bool EmailValidator(string address);

// Enum in file-scoped namespace
public enum EmailPriority
{
    Low,
    Normal,
    High,
    Critical
}

// Record in file-scoped namespace
public record Mailbox(string Address, string DisplayName);

// Record struct
public record struct AddressPair(string From, string To);

// Record class
public record class EmailTemplate(string Subject, string Body);

// Const in file-scoped namespace
public static class EmailConfig
{
    public const int MaxRetries = 3;
    public const string DefaultFrom = "noreply@example.com";
    // Multi-variable const
    public const int SmtpPort = 587, ImapPort = 993;
}

// Event in file-scoped namespace
public static class EmailEvents
{
    public static event EventHandler<string> OnEmailSent;
    public static event EventHandler<string> OnEmailFailed;
}

// Fields and Properties
public class Product
{
    // Fields (plain data members)
    public int Id;
    private string _name;
    protected double Price;

    // Properties (with accessor semantics)
    public string DisplayName { get; set; }
    public string Category { get; }
    public virtual string Description { get; set; }
}

public struct Coordinate
{
    public double Latitude;
    public double Longitude;
}
