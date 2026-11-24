import { AfterViewInit, ChangeDetectionStrategy, Component, ElementRef, inject, output, signal, viewChildren } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import {
  OtpState, otpBackspace, otpCode, otpCut, otpDelete, otpEmpty, otpMove, otpMoveTo, otpPaste, otpSelectAll, otpType,
} from '../../core/utils/otp.util';

/**
 * The six boxes of a pairing code: one digit per box, large, with a bottom border. Typing moves on to the next box,
 * Backspace goes back, the arrows move, a paste fills the boxes (six digits at once, spaces and other characters
 * dropped), Ctrl+A selects the whole code and Ctrl+X cuts it. The rules are the pure functions of `otp.util`; this
 * component only carries the events of the boxes to them.
 */
@Component({
  selector: 'app-otp-input',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="boxes" role="group" [attr.aria-label]="i18n.t('sync.join.title')">
      @for (digit of state().digits; track $index; let i = $index) {
        <input
          #box
          class="box"
          type="text"
          inputmode="numeric"
          spellcheck="false"
          [attr.autocomplete]="i === 0 ? 'one-time-code' : 'off'"
          [attr.aria-label]="i18n.t('sync.join.digit', { n: i + 1 })"
          [class.filled]="digit !== ''"
          [class.selected]="state().allSelected"
          [value]="digit"
          (keydown)="onKeydown($event, i)"
          (input)="onInput($event, i)"
          (paste)="onPaste($event, i)"
          (cut)="onCut($event, i)"
          (copy)="onCopy($event)"
          (focus)="onFocus(i)"
        />
      }
    </div>
  `,
  styles: [`
    :host { display: block; }
    .boxes { display: flex; justify-content: center; gap: 8px; }
    .box {
      flex: 1 1 0;
      min-width: 0;
      max-width: 40px;
      height: 54px;
      padding: 0;
      border: none;
      border-bottom: 2px solid var(--daftr-border);
      border-radius: 0;
      background: transparent;
      color: var(--daftr-text-strong);
      font-family: var(--daftr-font-mono);
      font-size: 28px;
      text-align: center;
      outline: none;
      caret-color: var(--daftr-accent);
      transition: border-color 0.1s, background 0.1s;
    }
    .box.filled { border-bottom-color: var(--daftr-text-soft); }
    .box:focus { border-bottom-color: var(--daftr-accent); }
    .box.selected { background: rgba(var(--daftr-hover-rgb), 0.1); border-bottom-color: var(--daftr-accent); }
  `],
})
export class OtpInputComponent implements AfterViewInit {
  protected readonly i18n = inject(I18nService);
  private readonly boxes = viewChildren<ElementRef<HTMLInputElement>>('box');

  /** The digits typed so far, joined: six of them make a complete code. */
  readonly codeChange = output<string>();

  protected readonly state = signal<OtpState>(otpEmpty());

  ngAfterViewInit(): void {
    this.boxes()[0]?.nativeElement.focus();
  }

  protected onKeydown(event: KeyboardEvent, index: number): void {
    if (event.ctrlKey || event.metaKey) {
      // Copy, cut and paste have their own events; only "select all" is the component's
      if (event.key.toLowerCase() === 'a') {
        event.preventDefault();
        this.state.update(otpSelectAll);
      }

      return;
    }

    switch (event.key) {
      case 'Backspace':
        return this.handle(event, otpBackspace(this.state(), index));
      case 'Delete':
        return this.handle(event, otpDelete(this.state(), index));
      case 'ArrowLeft':
        return this.handle(event, otpMove(this.state(), index, -1));
      case 'ArrowRight':
        return this.handle(event, otpMove(this.state(), index, 1));
      case 'Home':
        return this.handle(event, otpMoveTo(this.state(), 0));
      case 'End':
        return this.handle(event, otpMoveTo(this.state(), this.state().digits.length - 1));
    }

    // A printable character is a digit (typed here) or nothing: the boxes take digits only
    if (event.key.length === 1 && !event.altKey) {
      this.handle(event, otpType(this.state(), index, event.key));
    }
  }

  /** What `keydown` could not catch: autofill, a virtual keyboard, an input method. */
  protected onInput(event: Event, index: number): void {
    const field = event.target as HTMLInputElement;
    const previous = this.state().digits[index] ?? '';
    // A digit typed over a filled box: the box holds both, the new one is the other
    const typed = previous !== '' && field.value.length === 2 ? field.value.replace(previous, '') || previous : field.value;

    this.apply(otpType(this.state(), index, typed));
    field.value = this.state().digits[index] ?? '';
  }

  protected onPaste(event: ClipboardEvent, index: number): void {
    event.preventDefault();
    this.apply(otpPaste(this.state(), index, event.clipboardData?.getData('text') ?? ''));
  }

  protected onCut(event: ClipboardEvent, index: number): void {
    event.preventDefault();
    const cut = otpCut(this.state(), index);
    event.clipboardData?.setData('text/plain', cut.text);
    this.apply(cut.state);
  }

  /** Copy after a "select all": the whole code, not the digit of one box. */
  protected onCopy(event: ClipboardEvent): void {
    if (this.state().allSelected) {
      event.preventDefault();
      event.clipboardData?.setData('text/plain', otpCode(this.state()));
    }
  }

  /** A click or a Tab on a box puts the focus there, and drops a "select all". */
  protected onFocus(index: number): void {
    if (this.state().focus !== index || this.state().allSelected) {
      this.state.update(current => otpMoveTo(current, index));
    }
  }

  private handle(event: Event, next: OtpState): void {
    event.preventDefault();
    this.apply(next);
  }

  private apply(next: OtpState): void {
    const changed = next.digits.some((digit, position) => digit !== this.state().digits[position]);
    this.state.set(next);

    if (changed) {
      this.codeChange.emit(otpCode(next));
    }

    // The value of a box the state did not change may still differ from it (a refused character): written back
    this.boxes().forEach((box, position) => {
      box.nativeElement.value = next.digits[position] ?? '';
    });
    this.boxes()[next.focus]?.nativeElement.focus();
  }
}
