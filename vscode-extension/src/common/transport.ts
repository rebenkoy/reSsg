let transport_call: (v: any) => void;

export enum MessageType {
    getServerProcessStatus,
    serverProcessStatus,
    eventSave,
    saveStatus,
}

const global_dispatcher = {};

export abstract class Message {
	static type: MessageType;

    post() {
        transport_call({type: Object.getPrototypeOf(this).constructor.type, data: this});
    }

    static register_callback(cb: (msg) => void) {
        global_dispatcher[this.type] = cb;
    }

    static setTransport(call: (v: any) => void) {
        transport_call = call;
    }

    static process(msg: any) {
        console.log("Processing message:", msg);
        global_dispatcher[msg.type](msg.data);
    }

}

export class Ping extends Message {
	static override type = MessageType.getServerProcessStatus;
}

export class Pong extends Message {
	static override type = MessageType.serverProcessStatus;
	server_is_active: boolean;
    pull_req_url: string | undefined;
    constructor(server_is_active: boolean, pull_req_url: string | undefined) {
        super();
        this.server_is_active = server_is_active;
        this.pull_req_url = pull_req_url;
    }
}

export class TellSaveStatus extends Message {
	static override type = MessageType.saveStatus;
	ok: boolean;
	constructor(ok: boolean) {
        super();
		this.ok = ok;
	}
}

export class RequestSaveMessage extends Message {
    static override type = MessageType.eventSave;
    commit_message: string;
    constructor(commit_message: string) {
        super();
        this.commit_message = commit_message;
    }
}