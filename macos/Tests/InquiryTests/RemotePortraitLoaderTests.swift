import Foundation
import Testing
@testable import Inquiry

private final class DownloadCompletionProbe: @unchecked Sendable {
    private let lock = NSLock()
    private var storedOutcomes: [String] = []
    private var storedCancelCount = 0
    private var storedFinishCount = 0

    var outcomes: [String] {
        lock.withLock { storedOutcomes }
    }

    var cancelCount: Int {
        lock.withLock { storedCancelCount }
    }

    var finishCount: Int {
        lock.withLock { storedFinishCount }
    }

    func complete(_ result: Result<BoundedMediaDownloadState.Payload, any Error>) {
        lock.withLock {
            switch result {
            case .success:
                storedOutcomes.append("success")
            case .failure(let error) where error is CancellationError:
                storedOutcomes.append("cancelled")
            case .failure(let error):
                if let remoteError = error as? RemotePortraitError,
                   case .tooLarge = remoteError {
                    storedOutcomes.append("too-large")
                } else {
                    storedOutcomes.append("failure")
                }
            }
        }
    }

    func cancelTransport() {
        lock.withLock { storedCancelCount += 1 }
    }

    func finishTransport() {
        lock.withLock { storedFinishCount += 1 }
    }
}

private func acceptedResponse() -> HTTPURLResponse {
    HTTPURLResponse(
        url: URL(string: "https://upload.wikimedia.org/portrait.jpg")!,
        statusCode: 200,
        httpVersion: "HTTP/1.1",
        headerFields: ["Content-Type": "image/jpeg"]
    )!
}

@Test func portraitCancellationBeforeContinuationInstallCompletesOnceWithoutStarting() {
    let state = BoundedMediaDownloadState()
    let probe = DownloadCompletionProbe()
    state.cancel()

    let shouldStart = state.install(
        completion: probe.complete,
        cancelTransport: probe.cancelTransport,
        finishTransport: probe.finishTransport
    )
    state.cancel()
    state.complete(error: nil)

    #expect(!shouldStart)
    #expect(probe.outcomes == ["cancelled"])
    #expect(probe.cancelCount == 1)
    #expect(probe.finishCount == 0)
}

@Test func portraitOversizedSingleChunkFailsWithoutIntegerUnderflow() {
    let state = BoundedMediaDownloadState()
    let probe = DownloadCompletionProbe()
    let shouldStart = state.install(
        completion: probe.complete,
        cancelTransport: probe.cancelTransport,
        finishTransport: probe.finishTransport
    )
    #expect(shouldStart)
    #expect(state.accept(acceptedResponse()))

    let accepted = state.append(
        Data(repeating: 0, count: BoundedMediaDownload.maximumBytes + 1),
        maximumBytes: BoundedMediaDownload.maximumBytes
    )
    state.complete(error: nil)

    #expect(!accepted)
    #expect(probe.outcomes == ["too-large"])
    #expect(probe.cancelCount == 1)
    #expect(probe.finishCount == 0)
}

@Test func portraitCancellationAndCompletionRaceResumesExactlyOnce() async {
    for _ in 0..<200 {
        let state = BoundedMediaDownloadState()
        let probe = DownloadCompletionProbe()
        #expect(state.install(
            completion: probe.complete,
            cancelTransport: probe.cancelTransport,
            finishTransport: probe.finishTransport
        ))
        #expect(state.accept(acceptedResponse()))
        #expect(state.append(Data([0x01]), maximumBytes: BoundedMediaDownload.maximumBytes))

        await withTaskGroup(of: Void.self) { group in
            group.addTask { state.cancel() }
            group.addTask { state.complete(error: nil) }
        }

        #expect(probe.outcomes.count == 1)
        #expect(probe.cancelCount + probe.finishCount == 1)
    }
}

@Test func portraitLoadGenerationRejectsObsoleteCompletion() {
    var generation = PortraitLoadGeneration()
    let first = generation.advance()
    #expect(generation.isCurrent(first))

    let second = generation.advance()
    #expect(!generation.isCurrent(first))
    #expect(generation.isCurrent(second))
}
