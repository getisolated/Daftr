import { AfterViewInit, Directive, ElementRef, inject } from '@angular/core';

/** Focuses the input as soon as it is displayed and selects its content, ready to be overwritten (inline rename). */
@Directive({
  selector: 'input[appFocusSelect]',
  standalone: true,
})
export class FocusSelectDirective implements AfterViewInit {
  private readonly element = inject<ElementRef<HTMLInputElement>>(ElementRef);

  ngAfterViewInit(): void {
    this.element.nativeElement.focus();
    this.element.nativeElement.select();
  }
}
