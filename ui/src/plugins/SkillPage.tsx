import { BookOpen, ChevronLeft } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure, type Skill } from '../api';
import { danger } from '../components/Confirm';
import { Markdown } from '../components/Markdown';
import { Switch } from '../components/Switch';
import { desktop } from '../desktop';

interface Props {
  /** As the list has it now. */
  skill: Skill;
  onBack: () => void;
  onFailure: (failure: Failure) => void;
  onEnable: (skill: Skill, on: boolean) => void;
  onRemove: (skill: Skill, opener: HTMLElement | null) => void;
}

/** A skill's page: what it is, its folder, Enabled, and its SKILL.md to read. */
export function SkillPage({ skill, onBack, onFailure, onEnable, onRemove }: Props) {
  const { name } = skill;
  const [text, setText] = useState<string | null>(null);
  const remove = useRef<HTMLButtonElement>(null);
  // Read again each time the list changes: a change to its SKILL.md comes with one.
  const listed = JSON.stringify(skill);
  useEffect(() => {
    if (listed) api.skill(name).then((s) => setText(s.text), onFailure);
  }, [name, listed, onFailure]);
  return (
    <article className="skill-page">
      <button type="button" className="screen-back" onClick={onBack}>
        <ChevronLeft aria-hidden="true" />
        Plugins
      </button>
      <header className="screen-head">
        <span className="plugin-tile" aria-hidden="true">
          <BookOpen />
        </span>
        <div className="screen-title">
          <h1>{skill.name}</h1>
          {skill.problem ? (
            <p className="plugin-problem">{skill.problem}</p>
          ) : (
            <p className="muted">{skill.description}</p>
          )}
        </div>
        <div className="screen-actions">
          {desktop() && (
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => api.openSkill(name).catch(onFailure)}
            >
              Open folder
            </Button>
          )}
          <Button
            ref={remove}
            type="button"
            size="sm"
            variant="outline"
            className={danger}
            onClick={() => onRemove(skill, remove.current)}
          >
            Remove
          </Button>
        </div>
      </header>
      <p className="skill-folder">
        <span className="muted">Folder</span> <code>{skill.path}</code>
      </p>
      <div className="settingrow skill-enabled">
        <span className="skill-enabled-words">
          <span id="skill-enabled">Enabled</span>
          <span className="muted" id="skill-enabled-why">
            When it’s disabled, no agent can use it. It stays installed.
          </span>
        </span>
        <Switch
          on={skill.enabled}
          labelledBy="skill-enabled"
          describedBy="skill-enabled-why"
          onChange={(on) => onEnable(skill, on)}
        />
      </div>
      {text !== null && (
        <section className="skill-text">
          <h2>SKILL.md</h2>
          <Markdown text={text} />
        </section>
      )}
    </article>
  );
}
