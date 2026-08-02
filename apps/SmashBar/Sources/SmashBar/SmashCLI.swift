import Foundation

/// Bridge to the `smash` CLI. The CLI's `--json` output is SmashBar's entire
/// backend: no server, no sockets, no new API surface — the same reviewed
/// commands every other Smash surface uses.
enum SmashCLI {
    struct CLIError: Error, CustomStringConvertible {
        let message: String
        var description: String { message }
    }

    /// Workspace the app operates on: SMASH_WORKSPACE or ~/Smash,
    /// mirroring the CLI's own pathless-command fallback.
    static var workspace: String {
        if let env = ProcessInfo.processInfo.environment["SMASH_WORKSPACE"], !env.isEmpty {
            return (env as NSString).expandingTildeInPath
        }
        return (NSHomeDirectory() as NSString).appendingPathComponent("Smash")
    }

    /// Locate the smash launcher. Order: SMASH_CLI env, Homebrew paths, PATH.
    static func lnkPath() -> String? {
        if let env = ProcessInfo.processInfo.environment["SMASH_CLI"], !env.isEmpty {
            return env
        }
        let candidates = ["/opt/homebrew/bin/smash", "/usr/local/bin/smash"]
        for candidate in candidates where FileManager.default.isExecutableFile(atPath: candidate) {
            return candidate
        }
        let which = Process()
        which.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        which.arguments = ["smash"]
        let pipe = Pipe()
        which.standardOutput = pipe
        which.standardError = Pipe()
        try? which.run()
        which.waitUntilExit()
        let out = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return out.isEmpty ? nil : out
    }

    /// Run `smash <args>` and return stdout. Blocking — call off the main actor.
    static func run(_ args: [String]) throws -> Data {
        guard let smash = lnkPath() else {
            throw CLIError(message: "smash not found — install Smash (brew install ebrahimisoheil/smash/Smash) or set SMASH_CLI")
        }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: smash)
        process.arguments = args
        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr
        try process.run()
        let data = stdout.fileHandleForReading.readDataToEndOfFile()
        let errData = stderr.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        if process.terminationStatus != 0 && data.isEmpty {
            let message = String(data: errData, encoding: .utf8) ?? "smash exited \(process.terminationStatus)"
            throw CLIError(message: message.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        return data
    }

    /// Run an arbitrary executable + args (not the `smash` launcher) — used for
    /// the exact remediation commands `verify-mcp` emits (e.g. a venv pip
    /// upgrade). Blocking; call off the main actor.
    @discardableResult
    static func runRaw(_ command: [String]) throws -> Data {
        guard let executable = command.first else {
            throw CLIError(message: "empty command")
        }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = Array(command.dropFirst())
        let stdout = Pipe()
        process.standardOutput = stdout
        process.standardError = Pipe()
        try process.run()
        let data = stdout.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        return data
    }

    /// Fire-and-forget for long-lived processes (the local viewer).
    static func launchDetached(_ args: [String]) {
        guard let smash = lnkPath() else { return }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: smash)
        process.arguments = args
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try? process.run()
    }

    static func runJSON<T: Decodable>(_ type: T.Type, _ args: [String]) throws -> T {
        let data = try run(args)
        return try JSONDecoder().decode(type, from: data)
    }
}
