// tests/fixtures/java/ThrowsAbstract.java
// Tests that abstract/interface methods with throws clauses
// have complete signatures (including throws)

abstract class ThrowsAbstract {
    abstract void readData() throws IOException, SQLException;

    interface Processor {
        void process(String input) throws IllegalArgumentException;
    }
}
