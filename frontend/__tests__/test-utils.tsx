import React from 'react';
import { render, RenderOptions } from '@testing-library/react';
import { BrowserRouter, MemoryRouter } from 'react-router-dom';

interface WrapperProps {
  children: React.ReactNode;
}

/**
 * Custom render function that wraps component with BrowserRouter.
 * Use this for components that use routing hooks.
 */
export function renderWithRouter(ui: React.ReactElement, options?: Omit<RenderOptions, 'wrapper'>) {
  function Wrapper({ children }: WrapperProps) {
    return <BrowserRouter>{children}</BrowserRouter>;
  }

  return render(ui, { wrapper: Wrapper, ...options });
}

/**
 * Render with MemoryRouter for testing specific routes.
 */
export function renderWithMemoryRouter(
  ui: React.ReactElement,
  {
    initialEntries = ['/'],
    ...options
  }: Omit<RenderOptions, 'wrapper'> & { initialEntries?: string[] } = {}
) {
  function Wrapper({ children }: WrapperProps) {
    return <MemoryRouter initialEntries={initialEntries}>{children}</MemoryRouter>;
  }

  return render(ui, { wrapper: Wrapper, ...options });
}

// Re-export everything from testing-library
export * from '@testing-library/react';
export { default as userEvent } from '@testing-library/user-event';
