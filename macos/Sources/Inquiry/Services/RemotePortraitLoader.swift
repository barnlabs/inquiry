import AppKit
import Foundation
import ImageIO

enum RemotePortraitError: LocalizedError {
    case invalidURL
    case rejectedRedirect
    case invalidResponse
    case unsupportedMedia
    case tooLarge
    case invalidImage
    case dimensionsMismatch

    var errorDescription: String? {
        switch self {
        case .invalidURL: "The image URL did not pass the HTTPS host allowlist."
        case .rejectedRedirect: "The image redirected outside the approved media host."
        case .invalidResponse: "The image server returned an invalid response."
        case .unsupportedMedia: "The response was not a supported image."
        case .tooLarge: "The image exceeded Inquiry’s download or pixel limit."
        case .invalidImage: "The downloaded file could not be decoded as an image."
        case .dimensionsMismatch: "The decoded image’s aspect ratio did not match its cited metadata."
        }
    }
}

final class BoundedMediaDownloadState: @unchecked Sendable {
    typealias Payload = (Data, HTTPURLResponse)
    typealias Completion = (Result<Payload, any Error>) -> Void
    typealias TransportAction = () -> Void

    private struct CompletionAction {
        let completion: Completion
        let result: Result<Payload, any Error>
        let transportAction: TransportAction?

        func perform() {
            transportAction?()
            completion(result)
        }
    }

    private let lock = NSLock()
    private var data = Data()
    private var acceptedResponse: HTTPURLResponse?
    private var completion: Completion?
    private var cancelTransport: TransportAction?
    private var finishTransport: TransportAction?
    private var cancelled = false
    private var finished = false

    @discardableResult
    func install(
        completion: @escaping Completion,
        cancelTransport: @escaping TransportAction,
        finishTransport: @escaping TransportAction
    ) -> Bool {
        var completionAction: CompletionAction?
        let shouldStart = lock.withLock {
            guard !finished, self.completion == nil else {
                completionAction = CompletionAction(
                    completion: completion,
                    result: .failure(RemotePortraitError.invalidResponse),
                    transportAction: cancelTransport
                )
                return false
            }
            self.completion = completion
            self.cancelTransport = cancelTransport
            self.finishTransport = finishTransport
            if cancelled {
                completionAction = finishLocked(
                    with: .failure(CancellationError()),
                    cancelTransport: true
                )
                return false
            }
            return true
        }
        completionAction?.perform()
        return shouldStart
    }

    func accept(_ response: HTTPURLResponse) -> Bool {
        lock.withLock {
            guard isActive else { return false }
            acceptedResponse = response
            return true
        }
    }

    @discardableResult
    func append(_ chunk: Data, maximumBytes: Int) -> Bool {
        var completionAction: CompletionAction?
        let accepted = lock.withLock {
            guard isActive else { return false }
            guard chunk.count <= maximumBytes,
                  data.count <= maximumBytes - chunk.count else {
                completionAction = finishLocked(
                    with: .failure(RemotePortraitError.tooLarge),
                    cancelTransport: true
                )
                return false
            }
            data.append(chunk)
            return true
        }
        completionAction?.perform()
        return accepted
    }

    func reject(_ error: any Error) {
        finish(with: .failure(error), cancelTransport: true)
    }

    func complete(error: (any Error)?) {
        var completionAction: CompletionAction?
        lock.withLock {
            guard isActive else { return }
            if let error {
                completionAction = finishLocked(
                    with: .failure(error),
                    cancelTransport: true
                )
            } else if let acceptedResponse {
                completionAction = finishLocked(
                    with: .success((data, acceptedResponse)),
                    cancelTransport: false
                )
            } else {
                completionAction = finishLocked(
                    with: .failure(RemotePortraitError.invalidResponse),
                    cancelTransport: true
                )
            }
        }
        completionAction?.perform()
    }

    func cancel() {
        var completionAction: CompletionAction?
        lock.withLock {
            guard !finished else { return }
            cancelled = true
            completionAction = finishLocked(
                with: .failure(CancellationError()),
                cancelTransport: true
            )
        }
        completionAction?.perform()
    }

    private var isActive: Bool {
        !cancelled && !finished && completion != nil
    }

    private func finish(
        with result: Result<Payload, any Error>,
        cancelTransport: Bool
    ) {
        var completionAction: CompletionAction?
        lock.withLock {
            completionAction = finishLocked(
                with: result,
                cancelTransport: cancelTransport
            )
        }
        completionAction?.perform()
    }

    private func finishLocked(
        with result: Result<Payload, any Error>,
        cancelTransport shouldCancelTransport: Bool
    ) -> CompletionAction? {
        guard !finished, let completion else { return nil }
        finished = true
        self.completion = nil
        let transportAction = shouldCancelTransport ? cancelTransport : finishTransport
        cancelTransport = nil
        finishTransport = nil
        acceptedResponse = nil
        data = Data()
        return CompletionAction(
            completion: completion,
            result: result,
            transportAction: transportAction
        )
    }
}

final class BoundedMediaDownload: NSObject, URLSessionDataDelegate, URLSessionTaskDelegate, @unchecked Sendable {
    static let maximumBytes = 5_000_000
    static let allowedHosts = ["upload.wikimedia.org"]

    private let state = BoundedMediaDownloadState()

    static func isAllowed(_ url: URL?) -> Bool {
        guard let url,
              url.scheme == "https",
              url.user == nil,
              url.password == nil,
              url.port == nil else {
            return false
        }
        return url.host.map(allowedHosts.contains) == true
    }

    func download(_ url: URL) async throws -> (Data, HTTPURLResponse) {
        guard Self.isAllowed(url) else { throw RemotePortraitError.invalidURL }
        return try await withTaskCancellationHandler {
            try Task.checkCancellation()
            let configuration = URLSessionConfiguration.ephemeral
            configuration.httpCookieStorage = nil
            configuration.urlCredentialStorage = nil
            configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
            configuration.timeoutIntervalForRequest = 15
            configuration.timeoutIntervalForResource = 20
            configuration.waitsForConnectivity = false
            let queue = OperationQueue()
            queue.maxConcurrentOperationCount = 1
            let session = URLSession(
                configuration: configuration,
                delegate: self,
                delegateQueue: queue
            )
            var request = URLRequest(url: url)
            request.setValue("image/jpeg,image/png,image/webp", forHTTPHeaderField: "Accept")
            return try await withCheckedThrowingContinuation { continuation in
                let task = session.dataTask(with: request)
                let shouldStart = state.install(
                    completion: { result in continuation.resume(with: result) },
                    cancelTransport: {
                        task.cancel()
                        session.invalidateAndCancel()
                    },
                    finishTransport: { session.finishTasksAndInvalidate() }
                )
                if shouldStart {
                    task.resume()
                }
            }
        } onCancel: {
            self.cancel()
        }
    }

    func urlSession(
        _: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection _: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping (URLRequest?) -> Void
    ) {
        if Self.isAllowed(request.url) {
            completionHandler(request)
        } else {
            completionHandler(nil)
            state.reject(RemotePortraitError.rejectedRedirect)
        }
    }

    func urlSession(
        _: URLSession,
        dataTask _: URLSessionDataTask,
        didReceive response: URLResponse,
        completionHandler: @escaping (URLSession.ResponseDisposition) -> Void
    ) {
        guard let response = response as? HTTPURLResponse,
              response.statusCode == 200,
              Self.isAllowed(response.url)
        else {
            completionHandler(.cancel)
            state.reject(RemotePortraitError.invalidResponse)
            return
        }
        let mime = response.mimeType?.lowercased()
        guard ["image/jpeg", "image/png", "image/webp"].contains(mime) else {
            completionHandler(.cancel)
            state.reject(RemotePortraitError.unsupportedMedia)
            return
        }
        if response.expectedContentLength > Self.maximumBytes {
            completionHandler(.cancel)
            state.reject(RemotePortraitError.tooLarge)
            return
        }
        completionHandler(state.accept(response) ? .allow : .cancel)
    }

    func urlSession(_: URLSession, dataTask _: URLSessionDataTask, didReceive chunk: Data) {
        state.append(chunk, maximumBytes: Self.maximumBytes)
    }

    func urlSession(_: URLSession, task _: URLSessionTask, didCompleteWithError error: (any Error)?) {
        state.complete(error: error)
    }

    func cancel() {
        state.cancel()
    }
}

struct PortraitLoadGeneration: Sendable {
    private(set) var current: UInt64 = 0

    mutating func advance() -> UInt64 {
        current &+= 1
        return current
    }

    func isCurrent(_ candidate: UInt64) -> Bool {
        candidate == current
    }
}

@MainActor
final class RemotePortraitLoader: ObservableObject {
    enum State {
        case idle
        case loading
        case loaded(NSImage)
        case failed(String)
    }

    @Published private(set) var state: State = .idle
    private var task: Task<Void, Never>?
    private var downloader: BoundedMediaDownload?
    private var generation = PortraitLoadGeneration()

    func load(source: InquirySource) {
        guard case .idle = state, let url = source.provenance.previewUrl else { return }
        let loadGeneration = generation.advance()
        state = .loading
        let downloader = BoundedMediaDownload()
        self.downloader = downloader
        task = Task {
            defer {
                if generation.isCurrent(loadGeneration) {
                    self.downloader = nil
                    self.task = nil
                }
            }
            do {
                let (data, _) = try await downloader.download(url)
                try Task.checkCancellation()
                guard let imageSource = CGImageSourceCreateWithData(data as CFData, nil),
                      CGImageSourceGetCount(imageSource) == 1,
                      let properties = CGImageSourceCopyPropertiesAtIndex(imageSource, 0, nil)
                        as? [CFString: Any],
                      let width = (properties[kCGImagePropertyPixelWidth] as? NSNumber)?.intValue,
                      let height = (properties[kCGImagePropertyPixelHeight] as? NSNumber)?.intValue,
                      width > 0, height > 0 else {
                    throw RemotePortraitError.invalidImage
                }
                let (decodedPixels, overflow) = width.multipliedReportingOverflow(by: height)
                guard !overflow, decodedPixels <= 20_000_000 else {
                    throw RemotePortraitError.tooLarge
                }
                if let citedWidth = source.provenance.widthPixels,
                   let citedHeight = source.provenance.heightPixels,
                   citedWidth > 0, citedHeight > 0 {
                    let decodedRatio = Double(width) / Double(height)
                    let citedRatio = Double(citedWidth) / Double(citedHeight)
                    guard abs(decodedRatio - citedRatio) <= 0.02 else {
                        throw RemotePortraitError.dimensionsMismatch
                    }
                }
                let thumbnailOptions: [CFString: Any] = [
                    kCGImageSourceCreateThumbnailFromImageAlways: true,
                    kCGImageSourceCreateThumbnailWithTransform: true,
                    kCGImageSourceThumbnailMaxPixelSize: 2_048,
                    kCGImageSourceShouldCacheImmediately: true,
                ]
                guard let thumbnail = CGImageSourceCreateThumbnailAtIndex(
                    imageSource,
                    0,
                    thumbnailOptions as CFDictionary
                ) else {
                    throw RemotePortraitError.invalidImage
                }
                guard generation.isCurrent(loadGeneration) else { return }
                let image = NSImage(cgImage: thumbnail, size: .zero)
                state = .loaded(image)
            } catch is CancellationError {
                if generation.isCurrent(loadGeneration) {
                    state = .idle
                }
            } catch {
                if generation.isCurrent(loadGeneration) {
                    state = .failed(error.localizedDescription)
                }
            }
        }
    }

    func retry(source: InquirySource) {
        cancel()
        load(source: source)
    }

    func cancel() {
        _ = generation.advance()
        let activeDownloader = downloader
        let activeTask = task
        downloader = nil
        task = nil
        state = .idle
        activeDownloader?.cancel()
        activeTask?.cancel()
    }
}
