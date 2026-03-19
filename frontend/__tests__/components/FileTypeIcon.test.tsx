import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { FileTypeIcon } from '../../components/FileTypeIcon';

describe('FileTypeIcon', () => {
  it('renders npm icon for PackageJson type', () => {
    const { container } = render(<FileTypeIcon scriptType="PackageJson" />);
    expect(container.querySelector('svg')).toBeInTheDocument();
  });

  it('renders Rust icon for CargoToml type', () => {
    const { container } = render(<FileTypeIcon scriptType="CargoToml" />);
    expect(container.querySelector('svg')).toBeInTheDocument();
  });

  it('renders Docker icon for DockerCompose type', () => {
    const { container } = render(<FileTypeIcon scriptType="DockerCompose" />);
    expect(container.querySelector('svg')).toBeInTheDocument();
  });

  it('renders Dockerfile icon', () => {
    const { container } = render(<FileTypeIcon scriptType="Dockerfile" />);
    expect(container.querySelector('svg')).toBeInTheDocument();
  });

  it('renders Makefile icon', () => {
    const { container } = render(<FileTypeIcon scriptType="Makefile" />);
    expect(container.querySelector('svg')).toBeInTheDocument();
  });

  it('renders Shell script icon', () => {
    const { container } = render(<FileTypeIcon scriptType="ShellScript" />);
    expect(container.querySelector('svg')).toBeInTheDocument();
  });

  it('renders GitHub Actions icon', () => {
    const { container } = render(<FileTypeIcon scriptType="GithubActions" />);
    expect(container.querySelector('svg')).toBeInTheDocument();
  });

  it('renders Python icon', () => {
    const { container } = render(<FileTypeIcon scriptType="PythonProject" />);
    expect(container.querySelector('svg')).toBeInTheDocument();
  });

  it('renders generic icon for unknown type', () => {
    const { container } = render(<FileTypeIcon scriptType="UnknownType" />);
    expect(container.querySelector('svg')).toBeInTheDocument();
  });

  it('renders Go icon', () => {
    const { container } = render(<FileTypeIcon scriptType="GoMod" />);
    expect(container.querySelector('svg')).toBeInTheDocument();
  });
});
