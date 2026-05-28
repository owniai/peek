// core.cs — all kind classifications, scope paths, signature formats
// Each construct appears once; no duplicate coverage across core and edge.

namespace MyApp.Models
{
    // ── Class ──
    public class User
    {
        // ── Field ──
        private string _name;

        // ── Const ──
        public const int MaxAge = 150;

        // ── Constructor ──
        public User(string name) { }

        // ── Method ──
        public string GetName() { return _name; }

        // ── Property + Getter + Setter ──
        public string DisplayName { get; set; }

        // ── Event ──
        public event EventHandler OnChanged;
    }

    // ── Interface ──
    public interface IRepository
    {
        string GetName(int id);
    }

    // ── Enum + Variant ──
    public enum Status
    {
        Active
    }

    // ── Struct + Field ──
    public struct Point
    {
        public double X;
    }

    // ── Record ──
    public record Person(string FirstName, string LastName);

    // ── Delegate ──
    public delegate bool Validator(string input);
}

// ── File-scoped namespace ──
namespace MyApp.Services;

public class UserService
{
    public void CreateUser(string name) { }
}