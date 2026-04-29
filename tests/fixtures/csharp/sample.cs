// C# AST dump fixture — covers all target construct types

using System;
using System.Collections.Generic;

namespace MyApp.Models
{
    // --- class ---
    [Serializable]
    public class User
    {
        public string Name { get; set; }
        public int Age;

        // --- const ---
        public const int MaxAge = 150;
        private const string DefaultName = "Unknown";

        // --- event ---
        public event EventHandler<string> OnNameChanged;

        // --- method ---
        public string GetName()
        {
            return Name;
        }

        public static User Create(string name, int age)
        {
            return new User { Name = name, Age = age };
        }

        // --- delegate ---
        public delegate bool Validator(User user);
    }

    // --- struct ---
    public struct Point
    {
        public double X;
        public double Y;

        public double DistanceTo(Point other)
        {
            return Math.Sqrt(Math.Pow(X - other.X, 2) + Math.Pow(Y - other.Y, 2));
        }
    }

    // --- interface ---
    public interface IRepository<T>
    {
        T FindById(int id);
        void Save(T entity);
        event EventHandler<T> OnSaved;
    }

    // --- enum ---
    public enum Status
    {
        Active,
        Inactive,
        Suspended
    }

    // --- record (C# 9) ---
    public record Person(string FirstName, string LastName);

    // --- record struct (C# 10) ---
    public record struct PointRecord(double X, double Y);

    // --- record class (explicit) ---
    public record class Employee(string Name, string Department);

    // --- nested types ---
    public class Container
    {
        public class Inner
        {
            public void InnerMethod() { }
        }

        public struct InnerStruct
        {
            public int Value;
        }

        public enum InnerEnum { A, B, C }
    }

    // --- delegate at namespace level ---
    public delegate void NotifyHandler(string message);

    // --- event at namespace level (in a static class) ---
    public static class Events
    {
        public static event Action<string> GlobalNotify;
    }
}

// --- file-scoped namespace (C# 10+) ---
namespace MyApp.Services;

public class UserService
{
    public void CreateUser(string name) { }
}

// --- abstract class ---
public abstract class BaseEntity
{
    public abstract int Id { get; }

    public virtual void Validate()
    {
        Console.WriteLine("Validating...");
    }
}

// --- static class ---
public static class Helpers
{
    public static string FormatName(string name) => name.Trim().ToUpper();
}
