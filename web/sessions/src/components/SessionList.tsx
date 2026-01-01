import { For } from "solid-js";
import type { Session } from "../App";

interface Props {
  sessions: Session[];
  selectedId: string;
  onSelect: (id: string) => void;
}

export function SessionList(props: Props) {
  return (
    <ul class="session-list">
      <For each={props.sessions}>
        {(session) => (
          <li
            class="session-list__item"
            classList={{ "session-list__item--selected": session.id === props.selectedId }}
            onClick={() => props.onSelect(session.id)}
          >
            <span class="session-list__id" title={session.id}>
              {session.id.slice(0, 12)}
            </span>
            <span class="session-list__meta">
              <span class="session-list__format">{session.format}</span>
              <span class="session-list__age">{session.age}</span>
            </span>
          </li>
        )}
      </For>
    </ul>
  );
}
