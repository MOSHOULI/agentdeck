import { afterEach, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { Icon } from './Icon';

afterEach(cleanup);

it('keeps decorative icons out of the accessibility tree', () => {
  const { container } = render(<Icon name="agents" />);
  expect(container.querySelector('svg')).toHaveAttribute('aria-hidden', 'true');
  expect(container.querySelector('svg')).toHaveAttribute('focusable', 'false');
  expect(screen.queryByRole('img')).not.toBeInTheDocument();
});

it('supports named semantic icons and a safe fallback', () => {
  render(<Icon name="unrecognized" title="状态说明" size={24} />);
  expect(screen.getByRole('img', { name: '状态说明' })).toHaveAttribute('width', '24');
  expect(screen.getByRole('img')).not.toHaveAttribute('aria-hidden');
});
