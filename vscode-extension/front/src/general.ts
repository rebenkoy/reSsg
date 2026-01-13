// class StateManagerHandler {
//     get(target, name, reciever) {
//         const val = Reflect.get(target, 'value', reciever)[name];
//         const mng = Reflect.get(target, 'decl', reciever).update_managers[name];
//         return new Proxy(
//             new StateManager(val, mng),
//             new StateManagerHandler(),
//         );
//     }
//     set(target, name, value, reciever): boolean {
//         const val = Reflect.get(target, 'value', reciever)[name];
//         const cb = Reflect.get(target, 'decl', reciever).update_callbacks[name];
//         cb(val);
//         return true;
//     }
// }

// class StateGroupMarker {
//     static state_group_marker = true;
// }

// class StateManagerDecl<T> {
//     update_callbacks: {[K in keyof T]: (val: T[K]) => void };
//     update_managers: null | {[K in keyof T]: StateManagerDecl<T[K]>};
//     constructor(uc: {[K in keyof T]: (val: T[K]) => void }, um: null | {[K in keyof T]: StateManagerDecl<T[K]>}) {
//         this.update_callbacks = uc;
//         this.update_managers = um;
//     }
// }

// class StateManager<T> {
//     value: T;
//     decl: StateManagerDecl<T>;
//     constructor(val: T, decl?: StateManagerDecl<T>) {
//         this.value = val;
//         this.decl = decl;
//     }
// }

// class B {
//     c: number;
//     d: number;
//     constructor(c: number, d: number) {
//         this.c = c;
//         this.d = d;
//     }
// }
// class A {
//     a: number;
//     b: B;
//     constructor(a: number, b: B) {
//         this.a = a;
//         this.b = b;
//     }
// }

// const x = new StateManager(
//     new A(1, new B(2, 3)),
//     new StateManagerDecl<A>(
//         {
//             "a": (val: number) => {
//                 console.log(`Changing a for ${val}`);
//             },
//             "b": (val: {"c": number, "d": number}) => {
//                 console.log(`Changing b for ${val}`);
//             }
//         },
//         {
//             "a": null,
//             "b": new StateManagerDecl<B>({
//                 "c": (val: number) => {
//                     console.log(`Changing b.c for ${val}`);
//                 },
//                 "d": (val: number) => {
//                     console.log(`Changing b.d for ${val}`);
//                 }
//             }, {
//                 "c": null,
//                 "d": null
//             })
//         }
//     )
// );

(function () {
    const debounce = <T extends (...args: any[]) => any>(
        ms: number,
        fn: T,
    ): ((...args: Parameters<T>) => void) => {

        let timeoutId: ReturnType<typeof setTimeout>;

        return function (this: any, ...args: Parameters<T>): void {
            clearTimeout(timeoutId);
            timeoutId = setTimeout(() => {
            fn.apply(this, args);
            }, ms);
        };
    };
    class ServerState {        
        running = false;
        constructor(updates: Partial<ServerState> | undefined = undefined) {
            this.update(updates || {});
        }
        update(updates: Partial<ServerState>) {
            Object.assign(this, updates);
        }
    }
    class SaveState {
        branch_name = "";
        branch_name_disabled = false;
        commit_message = "";
        disabled = false;
        constructor(updates: Partial<SaveState> | undefined = undefined) {
            this.update(updates || {});
        }
        update(updates: Partial<SaveState>) {
            Object.assign(this, updates);
        }
    }
    class State {
        server: ServerState = new ServerState();
        save: SaveState = new SaveState();
        constructor(updates: Partial<State> | undefined = undefined) {
            this.update(updates || {});    
        }
        update(updates: Partial<State>) {
            for (const key in updates) {
                this[key].update(updates[key]);
            }
        }

        commit_update() {
            vscode.setState(this);
            draw_state(this);
        }
    }
    
    const save_button = document.getElementById('save-button')! as HTMLButtonElement;
    const branch_name_input = document.getElementById('branch-name')! as HTMLInputElement;
    const commit_message_input = document.getElementById('commit-message')! as HTMLTextAreaElement;
    const status_span = document.getElementById('status')! as HTMLSpanElement;
    const vscode = acquireVsCodeApi();
    const current_state: State = new State(vscode.getState() as Partial<State>);

    save_button.addEventListener('click', () => {
        save();
    });
    branch_name_input.addEventListener('keyup', (event) => {
        change_branch_name(event);
    });
    commit_message_input.addEventListener('keyup', (event) => {
        change_commit_message(event);
    });


    // Handle messages sent from the extension to the webview
    window.addEventListener('message', event => {
        const message = event.data; // The json data that the extension sent
        switch (message.type) {
            case 'setServerProcessStatus':
                {
                    setServerProcessStatus(message.is_active);
                    break;
                }
            case 'setBranch':
                {
                    setBranch(message.branch_name);
                    break;
                }
            case 'tellSaveStatus':
                {
                    tellSaveStatus(message.ok);
                    break;
                }
        }
    });
    
    function tellSaveStatus(ok: boolean) {
        clearSaveStatus();
        if (ok) {
            save_button.classList.add("save-button--ok");
        } else {
            save_button.classList.add("save-button--error");
        }
        scheduleClearSaveStatus();
    }

    function clearSaveStatus() {
        save_button.classList.remove(
            "save-button--ok",
            "save-button--loading",
            "save-button--error",
        );
    }
    const scheduleClearSaveStatus = debounce(1000, clearSaveStatus);

    function setServerProcessStatus(is_active: boolean) {
        current_state.server.running = is_active;
        current_state.commit_update();
    }

    function setBranch(branch_name: string) {
        branch_name_input.value = branch_name;
        branch_name_input.disabled = true;
        check_save_params(branch_name, true, undefined);
    }

    function change_branch_name(_: Event) {
        const branch_name = branch_name_input.value;
        check_save_params(branch_name, false, undefined);
    }

    function change_commit_message(_: Event) {
        const commit_message = commit_message_input.value;
        check_save_params(undefined, undefined, commit_message);
    }

    function check_save_params(branch_name?: string, branch_name_disabled?: boolean, commit_message?: string) {
        branch_name = branch_name || branch_name_input.value;
        commit_message = commit_message || commit_message_input.value;

        const active = branch_name 
            && branch_name !== 'master' 
            && commit_message 
            && commit_message.trim().length >= 10
        ;
        current_state.save.update({branch_name, commit_message, branch_name_disabled, disabled: !active});
        current_state.commit_update();
    }

    /** 
     * @param {string} color 
     */
    function save() {
        const {disabled, branch_name, commit_message} = current_state.save;
        if (disabled) {
            return;
        }
        save_button.classList.add("save-button--loading");
        scheduleClearSaveStatus();
        vscode.postMessage({ type: 'save', branch_name, commit_message });
    }

    let first_draw = true;

    const draw_state = debounce(100,
        (state: State) => {
            save_button.disabled = state.save.disabled;
            if (first_draw) {
                commit_message_input.value = state.save.commit_message;
                branch_name_input.value = state.save.branch_name;
                branch_name_input.disabled = state.save.branch_name_disabled;

                first_draw = false;
            }

            const new_status = state.server.running? 'running' : 'down';
            const color = state.server.running? 'green' : 'red';
            if (status_span === null) {
                return;
            }
            status_span.textContent = new_status;
            status_span.style['color'] = color;
        }
    );
    current_state.commit_update();
}());