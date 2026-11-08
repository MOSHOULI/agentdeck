import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { StrictMode } from 'react';
import * as bridge from '../bridge';
import { qwenDemoSummary } from '../qwenContract';
import { QwenSettings } from './QwenSettings';
const result = () => ({ summary: qwenDemoSummary(), synthetic: false });
beforeEach(() => {
  vi.spyOn(bridge, 'runtimeMode').mockReturnValue('native');
  vi.spyOn(bridge, 'inspectQwenSettings').mockResolvedValue(result());
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
function fill() {
  fireEvent.change(screen.getByLabelText('settings.json 绝对路径'), { target: { value: '/selected/settings.json' } });
  fireEvent.change(screen.getByLabelText('运行时 package.json 绝对路径'), { target: { value: '/runtime/package.json' } });
}
function submit() { fireEvent.submit(screen.getByRole('form', { name: 'Qwen 检查路径' })); }
function deferred() {
  let resolve!: (value: ReturnType<typeof result>) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<ReturnType<typeof result>>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
describe('Qwen settings workspace', () => {
  it('requires explicit paths and action, and only shows section metadata', async () => {
    render(<StrictMode><QwenSettings /></StrictMode>);
    expect(bridge.inspectQwenSettings).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: '检查所选文件' })).toBeDisabled();
    fill();
    expect(bridge.inspectQwenSettings).not.toHaveBeenCalled();
    submit();
    await screen.findByRole('region', { name: 'Qwen 检查结果' });
    expect(bridge.inspectQwenSettings).toHaveBeenCalledExactlyOnceWith('/selected/settings.json', '/runtime/package.json');
    expect(screen.getByText('通用：区段存在')).toBeInTheDocument();
    expect(screen.getAllByRole('button')).toHaveLength(2);
  });
  it('blocks repeated submissions and discards responses after path changes', async () => {
    const work = deferred(); vi.mocked(bridge.inspectQwenSettings).mockReturnValue(work.promise);
    render(<QwenSettings />); fill(); submit(); submit();
    expect(bridge.inspectQwenSettings).toHaveBeenCalledTimes(1);
    fireEvent.change(screen.getByLabelText('settings.json 绝对路径'), { target: { value: '/new/settings.json' } });
    await act(async () => { work.resolve(result()); });
    expect(screen.queryByRole('region')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '检查所选文件' })).toBeEnabled();
  });
  it('clear invalidates pending success and wipes paths', async () => {
    const work = deferred(); vi.mocked(bridge.inspectQwenSettings).mockReturnValue(work.promise);
    render(<QwenSettings />); fill(); submit();
    fireEvent.click(screen.getByRole('button', { name: '清除路径与结果' }));
    await act(async () => { work.resolve(result()); });
    expect(screen.queryByRole('region')).not.toBeInTheDocument();
    expect(screen.getByLabelText('settings.json 绝对路径')).toHaveValue('');
  });
  it('never echoes backend exceptions and supports retry', async () => {
    vi.mocked(bridge.inspectQwenSettings).mockRejectedValueOnce(new Error('SECRET'));
    render(<QwenSettings />); fill(); submit();
    expect(await screen.findByRole('alert')).not.toHaveTextContent('SECRET');
    submit(); await screen.findByRole('region'); expect(screen.queryByRole('alert')).toBeNull();
  });
  it('discarded errors and unmounted requests cannot restore results', async () => {
    const work = deferred(); vi.mocked(bridge.inspectQwenSettings).mockReturnValue(work.promise);
    const view = render(<QwenSettings />); fill(); submit(); view.unmount();
    await act(async () => { work.reject(new Error('SECRET')); });
    render(<QwenSettings />); expect(screen.queryByRole('alert')).toBeNull();
  });
  it('unavailable backend never dispatches; synthetic results are labeled', async () => {
    vi.mocked(bridge.runtimeMode).mockReturnValue('unavailable');
    const view = render(<QwenSettings />); submit(); expect(bridge.inspectQwenSettings).not.toHaveBeenCalled(); view.unmount();
    vi.mocked(bridge.runtimeMode).mockReturnValue('demo');
    vi.mocked(bridge.inspectQwenSettings).mockResolvedValue({ ...result(), synthetic: true });
    render(<QwenSettings />); fill(); submit();
    await screen.findByText('合成示例结果');
  });
});
