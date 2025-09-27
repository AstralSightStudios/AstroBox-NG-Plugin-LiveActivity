import Foundation
import ActivityKit

public struct ActivityContentTaskQueue: Codable, Sendable {
    public var id: String
    public var title: String
    public var text: String
    public var taskName: String
    public var taskType: String
    public var taskIcon: String
    public var state: [String: String]

    public init(id: String, title: String, text: String, taskName: String, taskType: String, taskIcon: String, state: [String: String]) {
        self.id = id
        self.title = title
        self.text = text
        self.taskName = taskName
        self.taskType = taskType
        self.taskIcon = taskIcon
        self.state = state
    }
}

public enum LiveActivityContent: Codable, Sendable {
    case taskQueue(ActivityContentTaskQueue)

    public enum CodingKeys: String, CodingKey {
        case type
        case data
    }

    public enum ContentType: String, Codable {
        case taskQueue = "TaskQueue"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(ContentType.self, forKey: .type)

        switch type {
        case .taskQueue:
            let data = try container.decode(ActivityContentTaskQueue.self, forKey: .data)
            self = .taskQueue(data)
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)

        switch self {
        case .taskQueue(let data):
            try container.encode(ContentType.taskQueue, forKey: .type)
            try container.encode(data, forKey: .data)
        }
    }
}

public struct CreateLiveActivityRequest: Decodable, Sendable {
    public let activityContentV: UInt32
    public let activityContent: LiveActivityContent

    public enum CodingKeys: String, CodingKey {
        case activityContentV = "activity_content_v"
        case activityContent = "activity_content"
    }

    public init(activityContentV: UInt32, activityContent: LiveActivityContent) {
        self.activityContentV = activityContentV
        self.activityContent = activityContent
    }
}

public struct UpdateLiveActivityRequest: Decodable, Sendable {
    public var state: [String: String]
}

public struct LiveActivityAttributes: ActivityAttributes {
    public struct ContentState: Codable, Hashable {
        public var stateItems: [String: String]
    }
    public var id: String
    public var type: LiveActivityContent.ContentType
    public var title: String
    public var text: String
    public var taskName: String
    public var taskType: String
    public var taskIcon: String
}

extension LiveActivityAttributes {
    public static var preview: LiveActivityAttributes {
        LiveActivityAttributes(
            id: "1",
            type: LiveActivityContent.ContentType.taskQueue,
            title: "任务执行中",
            text: "正在推送资源",
            taskName: "刘宸高清大头照",
            taskType: "快应用",
            taskIcon: ""
        )
    }

    public static var previewContentState: ContentState {
        .init(stateItems: [
            "percent": "91",
            "progress": "0.91",
        ])
    }
}
